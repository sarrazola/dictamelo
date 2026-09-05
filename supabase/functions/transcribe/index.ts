// Transcripción para usuarios Pro: recibe el audio, comprueba la licencia y llama a Groq
// con NUESTRA clave, que solo vive aquí como secreto del proyecto.

import { handler, jsonResponse, recordUsage, requireLicense, requireQuota } from "../_shared/license.ts";

const GROQ_URL = "https://api.groq.com/openai/v1/audio/transcriptions";
/** Modelos que aceptamos; evita que alguien pida uno caro por su cuenta. */
const ALLOWED_MODELS = new Set(["whisper-large-v3-turbo", "whisper-large-v3"]);
const DEFAULT_MODEL = "whisper-large-v3-turbo";
/** Igual que el límite de Groq. */
const MAX_BYTES = 25 * 1024 * 1024;

Deno.serve(handler(async (request, db) => {
  const license = await requireLicense(request, db);
  await requireQuota(license, db);

  const apiKey = Deno.env.get("GROQ_API_KEY");
  if (!apiKey) return jsonResponse({ error: "El servidor no tiene clave de transcripción" }, 500);

  const incoming = await request.formData();
  const file = incoming.get("file");
  if (!(file instanceof File)) return jsonResponse({ error: "Falta el audio" }, 400);
  if (file.size > MAX_BYTES) return jsonResponse({ error: "El audio supera los 25 MB" }, 413);

  const model = String(incoming.get("model") ?? DEFAULT_MODEL);
  const outgoing = new FormData();
  outgoing.set("file", file, file.name || "audio.wav");
  outgoing.set("model", ALLOWED_MODELS.has(model) ? model : DEFAULT_MODEL);
  outgoing.set("response_format", "verbose_json");
  outgoing.set("temperature", "0");
  for (const field of ["language", "prompt"]) {
    const value = incoming.get(field);
    if (typeof value === "string" && value.trim()) outgoing.set(field, value);
  }

  const response = await fetch(GROQ_URL, {
    method: "POST",
    headers: { Authorization: `Bearer ${apiKey}` },
    body: outgoing,
  });
  const text = await response.text();
  if (!response.ok) {
    // El error del proveedor va al registro; el cliente recibe algo entendible.
    console.error("Groq respondió", response.status, text.slice(0, 300));
    const status = response.status === 429 ? 429 : 502;
    const message = status === 429
      ? "El servicio está saturado; inténtalo en unos segundos"
      : "El servicio de transcripción falló";
    return jsonResponse({ error: message }, status);
  }

  const result = JSON.parse(text) as { duration?: number };
  await recordUsage(license, db, Number(result.duration ?? 0), "transcribe");
  return new Response(text, {
    headers: {
      "Content-Type": "application/json",
      "Access-Control-Allow-Origin": "*",
    },
  });
}));
