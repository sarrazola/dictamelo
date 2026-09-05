//! Conversión de cualquier audio que entienda Media Foundation (MP3, AAC/M4A, WMA, FLAC, WAV,
//! MP4/MOV…; viene con Windows, salvo en ediciones «N» sin el Media Feature Pack) a WAV PCM 16 bits
//! mono. Se pide 16 kHz al lector; si no puede remuestrear, se conserva la tasa original y el
//! resto de la app la convierte al leer el WAV.

use super::super::PlatformError;
use super::wide;
use std::path::Path;
use windows::core::PCWSTR;
use windows::Win32::Foundation::RPC_E_CHANGED_MODE;
use windows::Win32::Media::MediaFoundation::{
    IMFAttributes, IMFMediaType, IMFSample, MFAudioFormat_PCM, MFCreateMediaType, MFCreateSourceReaderFromURL,
    MFMediaType_Audio, MFShutdown, MFStartup, MFSTARTUP_NOSOCKET, MF_E_INVALIDMEDIATYPE, MF_E_INVALIDSTREAMNUMBER,
    MF_E_INVALID_FILE_FORMAT, MF_E_INVALID_FORMAT, MF_E_NO_MORE_TYPES, MF_E_TOPO_CODEC_NOT_FOUND,
    MF_E_UNSUPPORTED_BYTESTREAM_TYPE, MF_E_UNSUPPORTED_FORMAT, MF_MT_AUDIO_BITS_PER_SAMPLE, MF_MT_AUDIO_NUM_CHANNELS,
    MF_MT_AUDIO_SAMPLES_PER_SECOND, MF_MT_MAJOR_TYPE, MF_MT_SUBTYPE, MF_SOURCE_READERF_CURRENTMEDIATYPECHANGED,
    MF_SOURCE_READERF_ENDOFSTREAM, MF_SOURCE_READER_ALL_STREAMS, MF_SOURCE_READER_FIRST_AUDIO_STREAM, MF_VERSION,
};
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};

const TARGET_RATE: u32 = crate::audio::TARGET_SAMPLE_RATE;
const AUDIO_STREAM: u32 = MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32;
const ALL_STREAMS: u32 = MF_SOURCE_READER_ALL_STREAMS.0 as u32;

pub fn decode_audio_to_wav(input: &Path, output: &Path) -> Result<(), PlatformError> {
    // SAFETY: COM y Media Foundation se inicializan y liberan en este mismo hilo, emparejados.
    unsafe {
        let com = CoInitializeEx(None, COINIT_MULTITHREADED);
        if com.is_err() && com != RPC_E_CHANGED_MODE {
            return Err(PlatformError::Other(format!("COM: {}", com.message().trim())));
        }
        let result = match MFStartup(MF_VERSION, MFSTARTUP_NOSOCKET) {
            Ok(()) => {
                let result = decode(input, output);
                let _ = MFShutdown();
                result
            }
            Err(e) => Err(PlatformError::Other(format!("Media Foundation no está disponible: {}", e.message().trim()))),
        };
        if com.is_ok() {
            CoUninitialize();
        }
        result
    }
}

/// Traduce los errores de Media Foundation: formato/códec desconocido → `Unsupported`.
fn map_error(context: &str, e: windows::core::Error) -> PlatformError {
    const UNSUPPORTED: [windows::core::HRESULT; 8] = [
        MF_E_INVALIDMEDIATYPE,
        MF_E_INVALIDSTREAMNUMBER,
        MF_E_INVALID_FILE_FORMAT,
        MF_E_INVALID_FORMAT,
        MF_E_NO_MORE_TYPES,
        MF_E_TOPO_CODEC_NOT_FOUND,
        MF_E_UNSUPPORTED_BYTESTREAM_TYPE,
        MF_E_UNSUPPORTED_FORMAT,
    ];
    let message = format!("{context}: {} (0x{:08X})", e.message().trim(), e.code().0 as u32);
    if UNSUPPORTED.contains(&e.code()) {
        PlatformError::Unsupported(message)
    } else {
        PlatformError::Other(message)
    }
}

/// Tipo de salida PCM 16 bits; con `rate`, además mono a esa tasa (el lector remuestrea si puede).
unsafe fn pcm_type(rate: Option<u32>) -> windows::core::Result<IMFMediaType> {
    let media_type = MFCreateMediaType()?;
    media_type.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)?;
    media_type.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_PCM)?;
    media_type.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16)?;
    if let Some(rate) = rate {
        media_type.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, 1)?;
        media_type.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, rate)?;
    }
    Ok(media_type)
}

unsafe fn decode(input: &Path, output: &Path) -> Result<(), PlatformError> {
    let url = wide(&input.to_string_lossy());
    let reader = MFCreateSourceReaderFromURL(PCWSTR(url.as_ptr()), None::<&IMFAttributes>)
        .map_err(|e| map_error("no se pudo abrir el archivo", e))?;
    reader.SetStreamSelection(ALL_STREAMS, false).map_err(|e| map_error("selección de pistas", e))?;
    reader
        .SetStreamSelection(AUDIO_STREAM, true)
        .map_err(|e| map_error("el archivo no tiene pista de audio", e))?;

    let resampled = match pcm_type(Some(TARGET_RATE)) {
        Ok(media_type) => reader.SetCurrentMediaType(AUDIO_STREAM, None, &media_type).is_ok(),
        Err(_) => false,
    };
    if !resampled {
        let media_type = pcm_type(None).map_err(|e| map_error("tipo de salida", e))?;
        reader
            .SetCurrentMediaType(AUDIO_STREAM, None, &media_type)
            .map_err(|e| map_error("no se pudo decodificar a PCM", e))?;
    }
    let current = reader.GetCurrentMediaType(AUDIO_STREAM).map_err(|e| map_error("formato de salida", e))?;
    let channels = current.GetUINT32(&MF_MT_AUDIO_NUM_CHANNELS).map_err(|e| map_error("canales", e))?.max(1) as usize;
    let rate = current.GetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND).map_err(|e| map_error("tasa de muestreo", e))?;
    let bits = current.GetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE).map_err(|e| map_error("bits por muestra", e))?;
    if bits != 16 {
        return Err(PlatformError::Other(format!("formato PCM inesperado ({bits} bits por muestra)")));
    }
    log::info!(
        "Media Foundation: «{}» → PCM {rate} Hz, {channels} canal(es){}",
        input.display(),
        if resampled { ", remuestreado por el lector" } else { "" }
    );

    let spec = hound::WavSpec { channels: 1, sample_rate: rate, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
    let mut writer = hound::WavWriter::create(output, spec).map_err(|e| PlatformError::Other(e.to_string()))?;
    let mut frames = 0u64;
    loop {
        let mut flags = 0u32;
        let mut sample: Option<IMFSample> = None;
        reader
            .ReadSample(AUDIO_STREAM, 0, None, Some(&mut flags), None, Some(&mut sample))
            .map_err(|e| map_error("error al decodificar", e))?;
        if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
            break;
        }
        if flags & MF_SOURCE_READERF_CURRENTMEDIATYPECHANGED.0 as u32 != 0 {
            let changed = reader.GetCurrentMediaType(AUDIO_STREAM).map_err(|e| map_error("formato de salida", e))?;
            let same_rate = changed.GetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND).ok() == Some(rate);
            let same_channels = changed.GetUINT32(&MF_MT_AUDIO_NUM_CHANNELS).ok() == Some(channels as u32);
            if !same_rate || !same_channels {
                return Err(PlatformError::Other("el formato de audio cambió a mitad del archivo".into()));
            }
        }
        let Some(sample) = sample else { continue };
        let buffer = sample.ConvertToContiguousBuffer().map_err(|e| map_error("búfer de audio", e))?;
        let mut ptr: *mut u8 = std::ptr::null_mut();
        let mut len = 0u32;
        buffer.Lock(&mut ptr, None, Some(&mut len)).map_err(|e| map_error("bloqueo del búfer", e))?;
        let bytes = std::slice::from_raw_parts(ptr, len as usize);
        // PCM 16 bits little-endian intercalado por canal; se mezcla a mono promediando.
        let mut acc = 0i32;
        let mut in_frame = 0usize;
        let mut failed = None;
        for pair in bytes.chunks_exact(2) {
            acc += i32::from(i16::from_le_bytes([pair[0], pair[1]]));
            in_frame += 1;
            if in_frame == channels {
                if let Err(e) = writer.write_sample((acc / channels as i32) as i16) {
                    failed = Some(e);
                    break;
                }
                frames += 1;
                acc = 0;
                in_frame = 0;
            }
        }
        let _ = buffer.Unlock();
        if let Some(e) = failed {
            return Err(PlatformError::Other(format!("no se pudo escribir el WAV: {e}")));
        }
    }
    writer.finalize().map_err(|e| PlatformError::Other(format!("no se pudo cerrar el WAV: {e}")))?;
    if frames == 0 {
        return Err(PlatformError::Unsupported("el archivo no contiene audio decodificable".into()));
    }
    log::info!("Media Foundation: {frames} muestras mono ({:.1}s)", frames as f64 / f64::from(rate));
    Ok(())
}
