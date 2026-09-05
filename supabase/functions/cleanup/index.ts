// Limpieza del texto dictado para usuarios Pro. Mismo patrón que `transcribe`: la licencia
// manda y la clave del proveedor nunca sale del servidor.

import { handler, jsonResponse, recordUsage, requireLicense, requireQuota } from "../_shared/license.ts";

const GROQ_URL = "https://api.groq.com/openai/v1/chat/completions";
const ALLOWED_MODELS = new Set(["openai/gpt-oss-120b", "openai/gpt-oss-20b"]);
const DEFAULT_MODEL = "openai/gpt-oss-120b";
/** Un dictado no debería pasar de esto; corta textos absurdos. */
const MAX_CHARS = 20000;

interface Body {
  system?: string;
  text?: string;
  model?: string;
}

Deno.serve(handler(async (request, db) => {
  const license = await requireLicense(request, db);
  await requireQuota(license, db);

  const apiKey = Deno.env.get("GROQ_API_KEY");
  if (!apiKey) return jsonResponse({ error: "El servidor no tiene clave de limpieza" }, 500);

  const body = (await request.json()) as Body;
  const system = (body.system ?? "").trim();
  const text = (body.text ?? "").trim();
  if (!system || !text) return jsonResponse({ error: "Falta el texto" }, 400);
  if (text.length > MAX_CHARS) return jsonResponse({ error: "El texto es demasiado largo" }, 413);
  const model = ALLOWED_MODELS.has(body.model ?? "") ? body.model! : DEFAULT_MODEL;

  const response = await fetch(GROQ_URL, {
    method: "POST",
    headers: { Authorization: `Bearer ${apiKey}`, "Content-Type": "application/json" },
    body: JSON.stringify({
      model,
      temperature: 0.2,
      reasoning_effort: "low",
      messages: [
        { role: "system", content: system },
        { role: "user", content: text },
      ],
    }),
  });
  const raw = await response.text();
  if (!response.ok) {
    console.error("Groq respondió", response.status, raw.slice(0, 300));
    return jsonResponse({ error: "El servicio de limpieza falló" }, response.status === 429 ? 429 : 502);
  }

  // La limpieza se cobra como una fracción: cuesta mucho menos que transcribir.
  await recordUsage(license, db, Math.min(30, text.length / 200), "cleanup");
  return new Response(raw, {
    headers: { "Content-Type": "application/json", "Access-Control-Allow-Origin": "*" },
  });
}));
