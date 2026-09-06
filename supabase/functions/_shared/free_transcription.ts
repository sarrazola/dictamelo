import { Db, jsonResponse } from "./license.ts";
import { countWords, freeWavSeconds, reserveFree } from "./free.ts";
import { transcriptHash } from "./free_cleanup.ts";

// Hosted transcription keeps its economical model. Personal keys can select Large v3.
export const HOSTED_TRANSCRIPTION_MODEL = "whisper-large-v3-turbo";
const GROQ_URL = "https://api.groq.com/openai/v1/audio/transcriptions";

type Send = (url: string, init: RequestInit) => Promise<Response>;

/** Meter validated PCM bytes once, before the provider call. Uncertain outcomes retain time. */
export async function transcribeFree(
  db: Pick<Db, "rpc">,
  user: string,
  file: File,
  incoming: FormData,
  apiKey: string,
  send: Send = fetch,
): Promise<Response> {
  if (file.size > 4 * 1024 * 1024) {
    return jsonResponse({
      error: "Free recordings can be up to two minutes long.",
    }, 413);
  }
  const seconds = freeWavSeconds(new Uint8Array(await file.arrayBuffer()));
  const requestId = crypto.randomUUID();
  await reserveFree(db, user, requestId, seconds);
  const outgoing = new FormData();
  outgoing.set("file", file, file.name || "audio.wav");
  outgoing.set("model", HOSTED_TRANSCRIPTION_MODEL);
  outgoing.set("response_format", "verbose_json");
  outgoing.set("temperature", "0");
  for (const field of ["language", "prompt"]) {
    const value = incoming.get(field);
    if (typeof value === "string" && value.trim()) {
      outgoing.set(field, value.slice(0, field === "language" ? 16 : 1000));
    }
  }
  let response: Response;
  try {
    response = await send(GROQ_URL, {
      method: "POST",
      headers: { Authorization: `Bearer ${apiKey}` },
      body: outgoing,
      signal: AbortSignal.timeout(90000),
    });
  } catch {
    return jsonResponse({
      error: "The transcription service did not respond. Please try later.",
    }, 502);
  }
  if (!response.ok) {
    // Only a definite rejection proves no transcription took place. Keep attempts.
    if (response.status >= 400 && response.status < 500) {
      await db.rpc("finish_free_usage", {
        p_user: user,
        p_request: requestId,
        p_words: 0,
      });
    }
    return jsonResponse({
      error:
        "The transcription service is temporarily unavailable. Please try later.",
    }, response.status === 429 ? 429 : 502);
  }
  let result: { text?: unknown; duration?: unknown };
  try {
    result = await response.json();
  } catch {
    return jsonResponse({
      error: "The transcription service returned an invalid response.",
    }, 502);
  }
  if (
    !result || typeof result.text !== "string" ||
    result.text.trim().length > 20000
  ) {
    return jsonResponse({
      error: "The transcription service returned an invalid transcript.",
    }, 502);
  }
  const cleanupReceipt = await db.rpc("finish_free_transcription", {
    p_user: user,
    p_request: requestId,
    p_words: countWords(result.text),
    p_transcript_hash: await transcriptHash(result.text),
  });
  return jsonResponse({
    ...result,
    text: result.text.trim(),
    duration: seconds,
    cleanupReceipt,
  });
}
