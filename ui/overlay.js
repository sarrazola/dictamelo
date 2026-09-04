// Indicador flotante: solo refleja los eventos que emite Rust.
const { listen } = window.__TAURI__.event;
const { invoke } = window.__TAURI__.core;

const LABELS = {
  idle: "Listo",
  recording: "Grabando… suelta para transcribir",
  transcribing: "Transcribiendo…",
  pasting: "Pegando…",
};
const bars = Array.from(document.querySelectorAll(".bars i"));
let smoothed = 0;

function render(status) {
  const pill = document.getElementById("pill");
  pill.className = `pill ${status.state}`;
  document.getElementById("label").textContent = LABELS[status.state] || status.message || status.state;
  if (status.state !== "recording") bars.forEach((b) => (b.style.height = "4px"));
}

listen("status", (e) => render(e.payload));
listen("audio-level", (e) => {
  // RMS típico de voz: 0.02–0.3. Curva suave para que el movimiento sea visible.
  const level = Math.min(1, Math.sqrt(Math.max(0, e.payload)) * 2.2);
  smoothed = smoothed * 0.5 + level * 0.5;
  bars.forEach((bar, i) => {
    const weight = [0.55, 0.8, 1, 0.8, 0.55][i];
    const jitter = 0.85 + Math.random() * 0.3;
    bar.style.height = `${4 + Math.round(smoothed * weight * jitter * 12)}px`;
  });
});
invoke("get_status").then(render).catch(() => {});
invoke("ui_ready").catch(() => {});
