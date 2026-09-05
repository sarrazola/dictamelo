//! Grabador basado en cpal. El stream vive en un hilo dedicado porque `cpal::Stream`
//! no es `Send`; el resto de la app se comunica con él por canales.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample, StreamConfig};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use tokio::sync::oneshot;

#[derive(Debug, Clone, thiserror::Error)]
pub enum AudioError {
    #[error("No se encontró ningún micrófono")]
    NoDevice,
    #[error("No se encontró el micrófono «{0}»; revisa la configuración")]
    DeviceNotFound(String),
    #[error("Sin permiso para usar el micrófono")]
    PermissionDenied,
    #[error("No se pudo abrir el micrófono: {0}")]
    Open(String),
    #[error("Error durante la grabación: {0}")]
    Stream(String),
    #[error("No hay ninguna grabación en curso")]
    NotRecording,
    #[error("Ya hay una grabación en curso")]
    AlreadyRecording,
    #[error("El hilo de audio no responde")]
    Unavailable,
}

impl AudioError {
    /// Mensaje para el usuario en el idioma indicado (el `Display` en español va al registro).
    pub fn localized(&self, lang: &str) -> String {
        use crate::i18n::{t, tf};
        match self {
            AudioError::NoDevice => t(lang, "audio.no_device").into(),
            AudioError::DeviceNotFound(d) => tf(lang, "audio.device_not_found", &[("d", d)]),
            AudioError::PermissionDenied => t(lang, "audio.permission").into(),
            AudioError::Open(e) => tf(lang, "audio.open", &[("e", e)]),
            AudioError::Stream(e) => tf(lang, "audio.stream", &[("e", e)]),
            AudioError::NotRecording | AudioError::AlreadyRecording | AudioError::Unavailable => {
                t(lang, "audio.unavailable").into()
            }
        }
    }
}

/// Muestras tal como las entrega el dispositivo (intercaladas por canal, f32 en [-1, 1]).
#[derive(Debug, Clone)]
pub struct RawRecording {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
}

enum Command {
    Start { device: Option<String>, reply: oneshot::Sender<Result<(), AudioError>> },
    Stop { reply: oneshot::Sender<Result<RawRecording, AudioError>> },
    Cancel,
}

pub struct Recorder {
    tx: mpsc::Sender<Command>,
    level: Arc<AtomicU32>,
    recording: Arc<AtomicBool>,
}

impl Recorder {
    /// Arranca el hilo de audio (se crea una sola vez por app).
    pub fn spawn() -> Recorder {
        let (tx, rx) = mpsc::channel();
        let level = Arc::new(AtomicU32::new(0));
        let recording = Arc::new(AtomicBool::new(false));
        let thread_level = level.clone();
        let thread_flag = recording.clone();
        std::thread::Builder::new()
            .name("dictado-audio".into())
            .spawn(move || audio_thread(rx, thread_level, thread_flag))
            .expect("no se pudo crear el hilo de audio");
        Recorder { tx, level, recording }
    }

    pub async fn start(&self, device: Option<String>) -> Result<(), AudioError> {
        let (reply, rx) = oneshot::channel();
        self.tx
            .send(Command::Start { device, reply })
            .map_err(|_| AudioError::Unavailable)?;
        rx.await.map_err(|_| AudioError::Unavailable)?
    }

    pub async fn stop(&self) -> Result<RawRecording, AudioError> {
        let (reply, rx) = oneshot::channel();
        self.tx
            .send(Command::Stop { reply })
            .map_err(|_| AudioError::Unavailable)?;
        rx.await.map_err(|_| AudioError::Unavailable)?
    }

    /// Descarta la grabación en curso (si la hay).
    pub fn cancel(&self) {
        let _ = self.tx.send(Command::Cancel);
    }

    pub fn is_recording(&self) -> bool {
        self.recording.load(Ordering::SeqCst)
    }

    /// Nivel RMS del último bloque capturado, en [0, 1].
    pub fn level(&self) -> f32 {
        f32::from_bits(self.level.load(Ordering::Relaxed))
    }
}

struct ActiveStream {
    _stream: cpal::Stream,
    buffer: Arc<Mutex<Vec<f32>>>,
    error: Arc<Mutex<Option<String>>>,
    sample_rate: u32,
    channels: u16,
}

fn audio_thread(rx: mpsc::Receiver<Command>, level: Arc<AtomicU32>, recording: Arc<AtomicBool>) {
    let mut active: Option<ActiveStream> = None;
    while let Ok(cmd) = rx.recv() {
        match cmd {
            Command::Start { device, reply } => {
                if active.is_some() {
                    let _ = reply.send(Err(AudioError::AlreadyRecording));
                    continue;
                }
                match open_stream(device.as_deref(), level.clone()) {
                    Ok(stream) => {
                        active = Some(stream);
                        recording.store(true, Ordering::SeqCst);
                        let _ = reply.send(Ok(()));
                    }
                    Err(e) => {
                        let _ = reply.send(Err(e));
                    }
                }
            }
            Command::Stop { reply } => {
                recording.store(false, Ordering::SeqCst);
                level.store(0, Ordering::Relaxed);
                let result = match active.take() {
                    Some(stream) => finish(stream),
                    None => Err(AudioError::NotRecording),
                };
                let _ = reply.send(result);
            }
            Command::Cancel => {
                recording.store(false, Ordering::SeqCst);
                level.store(0, Ordering::Relaxed);
                active = None;
            }
        }
    }
}

fn finish(stream: ActiveStream) -> Result<RawRecording, AudioError> {
    // Al soltar el stream se detiene la captura; después leemos el búfer.
    drop(stream._stream);
    if let Some(err) = stream.error.lock().unwrap_or_else(|e| e.into_inner()).take() {
        return Err(AudioError::Stream(err));
    }
    let samples = std::mem::take(&mut *stream.buffer.lock().unwrap_or_else(|e| e.into_inner()));
    Ok(RawRecording { samples, sample_rate: stream.sample_rate, channels: stream.channels })
}

fn map_cpal(e: cpal::Error) -> AudioError {
    use cpal::ErrorKind;
    match e.kind() {
        ErrorKind::PermissionDenied => AudioError::PermissionDenied,
        ErrorKind::DeviceNotAvailable => AudioError::NoDevice,
        _ => AudioError::Open(e.to_string()),
    }
}

fn open_stream(device_name: Option<&str>, level: Arc<AtomicU32>) -> Result<ActiveStream, AudioError> {
    let host = cpal::default_host();
    let device = match device_name {
        Some(name) => host
            .input_devices()
            .map_err(map_cpal)?
            .find(|d| d.description().map(|desc| desc.name() == name).unwrap_or(false))
            .ok_or_else(|| AudioError::DeviceNotFound(name.to_string()))?,
        None => host.default_input_device().ok_or(AudioError::NoDevice)?,
    };
    let device_label = device
        .description()
        .map(|d| d.name().to_string())
        .unwrap_or_else(|_| "desconocido".into());

    let supported = device.default_input_config().map_err(map_cpal)?;
    let config: StreamConfig = supported.config();
    let sample_rate = config.sample_rate;
    let channels = config.channels;
    log::info!(
        "Micrófono «{device_label}»: {sample_rate} Hz, {channels} canal(es), {:?}",
        supported.sample_format()
    );

    // Reservamos ~30 s para evitar reasignaciones en el hilo de audio.
    let buffer = Arc::new(Mutex::new(Vec::with_capacity(
        sample_rate as usize * channels as usize * 30,
    )));
    let error = Arc::new(Mutex::new(None));

    let stream = match supported.sample_format() {
        SampleFormat::F32 => build::<f32>(&device, config, &buffer, &error, level),
        SampleFormat::I16 => build::<i16>(&device, config, &buffer, &error, level),
        SampleFormat::I32 => build::<i32>(&device, config, &buffer, &error, level),
        SampleFormat::U16 => build::<u16>(&device, config, &buffer, &error, level),
        SampleFormat::U8 => build::<u8>(&device, config, &buffer, &error, level),
        SampleFormat::I8 => build::<i8>(&device, config, &buffer, &error, level),
        SampleFormat::F64 => build::<f64>(&device, config, &buffer, &error, level),
        other => Err(AudioError::Open(format!("formato de muestra no soportado: {other:?}"))),
    }?;
    stream.play().map_err(map_cpal)?;

    Ok(ActiveStream { _stream: stream, buffer, error, sample_rate, channels })
}

fn build<T>(
    device: &cpal::Device,
    config: StreamConfig,
    buffer: &Arc<Mutex<Vec<f32>>>,
    error: &Arc<Mutex<Option<String>>>,
    level: Arc<AtomicU32>,
) -> Result<cpal::Stream, AudioError>
where
    T: SizedSample + Sample,
    f32: FromSample<T>,
{
    let buffer = buffer.clone();
    let error = error.clone();
    device
        .build_input_stream::<T, _, _>(
            config,
            move |data: &[T], _| {
                let mut buf = buffer.lock().unwrap_or_else(|e| e.into_inner());
                let mut energy = 0f32;
                for &sample in data {
                    let v: f32 = sample.to_sample();
                    energy += v * v;
                    buf.push(v);
                }
                let rms = (energy / data.len().max(1) as f32).sqrt();
                level.store(rms.to_bits(), Ordering::Relaxed);
            },
            move |err| {
                // Una discontinuidad del búfer (xrun) es un pequeño salto en el audio, no el fin de la
                // captura: WASAPI (Windows) la avisa con frecuencia al arrancar y el stream sigue vivo.
                if matches!(err.kind(), cpal::ErrorKind::Xrun) {
                    log::warn!("Aviso del stream de audio (se sigue grabando): {err}");
                    return;
                }
                log::error!("Error en el stream de audio: {err}");
                *error.lock().unwrap_or_else(|e| e.into_inner()) = Some(err.to_string());
            },
            None,
        )
        .map_err(map_cpal)
}
