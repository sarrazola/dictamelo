import { Db, LicenseError } from "./license.ts";

export async function requireUser(request: Request): Promise<{ id: string; email: string }> {
  const authorization = request.headers.get("authorization");
  if (!authorization?.startsWith("Bearer ")) throw new LicenseError(401, "Sign in to use your free words.");
  const response = await fetch(`${Deno.env.get("SUPABASE_URL")}/auth/v1/user`, {
    headers: { Authorization: authorization, apikey: Deno.env.get("SUPABASE_ANON_KEY")! },
    signal: AbortSignal.timeout(10000),
  });
  if (!response.ok) throw new LicenseError(response.status >= 500 ? 503 : 401, "Please sign in again.");
  const user = await response.json();
  if (!user.id || !user.email || !user.email_confirmed_at || user.is_anonymous) {
    throw new LicenseError(403, "Verify your email to use the free plan.");
  }
  return { id: user.id, email: user.email };
}

export function countWords(text: string): number {
  // ICU word segmentation supports all dictation languages; punctuation alone is not usage.
  const segmenter = new Intl.Segmenter(undefined, { granularity: "word" });
  return [...segmenter.segment(text)].filter((part) => part.isWordLike).length;
}

/** Free uploads are bounded, real PCM WAV recordings; duration never comes from the client. */
export function freeWavSeconds(bytes: Uint8Array): number {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const tag = (offset: number) => new TextDecoder().decode(bytes.subarray(offset, offset + 4));
  if (bytes.length < 44 || tag(0) !== "RIFF" || tag(8) !== "WAVE") {
    throw new LicenseError(400, "The free plan accepts WAV recordings up to two minutes. Use Pro or your own key for other files.");
  }
  let rate = 0, size = 0;
  for (let offset = 12; offset + 8 <= bytes.length;) {
    const length = view.getUint32(offset + 4, true);
    if (offset + 8 + length > bytes.length) throw new LicenseError(400, "Invalid WAV recording.");
    if (tag(offset) === "fmt ") {
      if (length < 16 || view.getUint16(offset + 8, true) !== 1 ||
        view.getUint16(offset + 10, true) !== 1 || view.getUint32(offset + 12, true) !== 16000 ||
        view.getUint32(offset + 16, true) !== 32000 || view.getUint16(offset + 20, true) !== 2 ||
        view.getUint16(offset + 22, true) !== 16) throw new LicenseError(400, "Use a mono 16 kHz PCM WAV recording.");
      rate = 32000;
    }
    if (tag(offset) === "data") size += length;
    offset += 8 + length + (length % 2);
  }
  if (!rate || !size || size / rate > 120.1) throw new LicenseError(413, "Free recordings can be up to two minutes long.");
  return size / rate;
}

export async function reserveFree(db: Db, user: string, request: string): Promise<void> {
  await db.rpc("reserve_free_usage", { p_user: user, p_request: request });
}
