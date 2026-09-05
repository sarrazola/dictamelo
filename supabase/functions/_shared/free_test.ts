import { countWords, freeWavSeconds } from "./free.ts";

function assert(value: boolean, message = "Assertion failed") { if (!value) throw new Error(message); }
function wav(seconds: number): Uint8Array {
  const b = new Uint8Array(44 + Math.round(seconds * 32000));
  const v = new DataView(b.buffer);
  const text = (at: number, s: string) => b.set(new TextEncoder().encode(s), at);
  text(0, "RIFF"); v.setUint32(4, b.length - 8, true); text(8, "WAVE"); text(12, "fmt ");
  v.setUint32(16, 16, true); v.setUint16(20, 1, true); v.setUint16(22, 1, true);
  v.setUint32(24, 16000, true); v.setUint32(28, 32000, true); v.setUint16(32, 2, true); v.setUint16(34, 16, true);
  text(36, "data"); v.setUint32(40, b.length - 44, true);
  return b;
}
function rejects(bytes: Uint8Array) { let failed = false; try { freeWavSeconds(bytes); } catch { failed = true; } assert(failed); }
Deno.test("word counting ignores punctuation and whitespace", () => {
  assert(countWords(" Hola,   mundo!\nEsto es una prueba. ") === 6);
  assert(countWords("... \n") === 0);
  assert(countWords("don't stop") === 2);
  assert(countWords("你好世界") > 0);
});
Deno.test("accept real PCM at the duration boundary", () => {
  assert(freeWavSeconds(wav(120)) === 120);
  rejects(wav(121));
});
Deno.test("reject forged duration, truncated data and other formats", () => {
  const forged = wav(121); new DataView(forged.buffer).setUint32(28, 64000, true); rejects(forged);
  rejects(wav(1).subarray(0, 50));
  rejects(new TextEncoder().encode("not an audio recording"));
  const stereo = wav(1); new DataView(stereo.buffer).setUint16(22, 2, true); rejects(stereo);
});
