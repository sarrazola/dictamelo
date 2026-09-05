// Transcripción para usuarios Pro: recibe el audio, comprueba la licencia y llama a Groq
// con NUESTRA clave, que solo vive aquí como secreto del proyecto.

import { handler, jsonResponse, recordUsage, requireLicense, requireQuota } from "../_shared/license.ts";

import { countWords, freeWavSeconds, requireUser, reserveFree } from "../_shared/free.ts";

const GROQ_URL = "https://api.groq.com/openai/v1/audio/transcriptions";
/** Modelos que aceptamos; evita que alguien pida uno caro por su cuenta. */
const ALLOWED_MODELS = new Set(["whisper-large-v3-turbo", "whisper-large-v3"]);
const DEFAULT_MODEL = "whisper-large-v3-turbo";
/** Igual que el límite de Groq. */
const MAX_BYTES = 25 * 1024 * 1024;

Deno.serve(handler(async (request, db) => {
  const freeUser = request.headers.has("x-license-key") ? null : await requireUser(request);
  const license = freeUser ? null : await requireLicense(request, db);
  if (license) await requireQuota(license, db);

  const apiKey = Deno.env.get("GROQ_API_KEY");
  if (!apiKey) return jsonResponse({ error: "El servidor no tiene clave de transcripción" }, 500);

  const incoming = await request.formData();
  const file = incoming.get("file");
  if (!(file instanceof File)) return jsonResponse({ error: "Falta el audio" }, 400);
  if (file.size > MAX_BYTES) return jsonResponse({ error: "El audio supera los 25 MB" }, 413);

  if (freeUser) {
    if (file.size > 4 * 1024 * 1024) return jsonResponse({ error: "Free recordings can be up to two minutes long." }, 413);
    freeWavSeconds(new Uint8Array(await file.arrayBuffer()));
  }
  const requestId = crypto.randomUUID();
  if (freeUser) await reserveFree(db, freeUser.id, requestId);
  let completed = false;
  try {
  const model = String(incoming.get("model") ?? DEFAULT_MODEL);
  const outgoing = new FormData();
  outgoing.set("file", file, file.name || "audio.wav");
  outgoing.set("model", !freeUser && ALLOWED_MODELS.has(model) ? model : DEFAULT_MODEL);
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
    signal: AbortSignal.timeout(90000),
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

  const result = JSON.parse(text) as { duration?: number; text?: string };
  if (typeof result.text !== "string") return jsonResponse({ error: "The transcription service returned an invalid response." }, 502);
  if (freeUser) {
    await db.rpc("finish_free_usage", { p_user: freeUser.id, p_request: requestId, p_words: countWords(result.text) });
    completed = true;
  }
  if (license) await recordUsage(license, db, Number(result.duration ?? 0), "transcribe");
  return new Response(text, {
    headers: {
      "Content-Type": "application/json",
      "Access-Control-Allow-Origin": "*",
    },
  });
  } finally {
    if (freeUser && !completed) {
      try { await db.rpc("finish_free_usage", { p_user: freeUser.id, p_request: requestId, p_words: 0 }); }
      catch { console.error("Could not release free quota reservation; it expires automatically."); }
    }
  }
}));
