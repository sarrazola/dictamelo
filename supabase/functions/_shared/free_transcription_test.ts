import {
  HOSTED_TRANSCRIPTION_MODEL,
  transcribeFree,
} from "./free_transcription.ts";
import { LicenseError } from "./license.ts";

function assert(value: unknown, message = "Assertion failed"): asserts value {
  if (!value) throw new Error(message);
}
const USER = "20000000-0000-4000-8000-000000000002";
function wav(): Uint8Array<ArrayBuffer> {
  const b = new Uint8Array(44 + 187360);
  const v = new DataView(b.buffer);
  for (
    const [at, text] of [[0, "RIFF"], [8, "WAVE"], [12, "fmt "], [
      36,
      "data",
    ]] as const
  ) b.set(new TextEncoder().encode(text), at);
  v.setUint32(4, b.length - 8, true);
  v.setUint32(16, 16, true);
  v.setUint16(20, 1, true);
  v.setUint16(22, 1, true);
  v.setUint32(24, 16000, true);
  v.setUint32(28, 32000, true);
  v.setUint16(32, 2, true);
  v.setUint16(34, 16, true);
  v.setUint32(40, 187360, true);
  return b;
}
function fixture() {
  return new File([wav()], "english.wav", { type: "audio/wav" });
}
function recorder() {
  const calls: Array<{ name: string; body: Record<string, unknown> }> = [];
  return {
    calls,
    rpc(name: string, body: Record<string, unknown>) {
      calls.push({ name, body });
      return Promise.resolve(
        name === "finish_free_transcription" ? body.p_request : null,
      );
    },
  };
}
Deno.test("free transcription measures PCM, ignores forged duration/model and settles once", async () => {
  const db = recorder(), input = new FormData();
  input.set("duration", "0.001");
  input.set("model", "untrusted-model");
  const response = await transcribeFree(
    db,
    USER,
    fixture(),
    input,
    "synthetic",
    (_url, init) => {
      assert(db.calls.length === 1 && db.calls[0].body.p_seconds === 5.855);
      assert(
        (init.body as FormData).get("model") === HOSTED_TRANSCRIPTION_MODEL,
      );
      return Promise.resolve(
        new Response(
          JSON.stringify({ text: " hello world ", duration: 0.001 }),
        ),
      );
    },
  );
  const body = await response.json();
  assert(
    response.status === 200 && body.duration === 5.855 &&
      body.text === "hello world",
  );
  assert(
    db.calls.length === 2 && db.calls[1].name === "finish_free_transcription" &&
      db.calls[1].body.p_words === 2,
  );
  assert(body.cleanupReceipt === db.calls[0].body.p_request);
});
Deno.test("audio quota refusal and malformed PCM never call the transcription provider", async () => {
  let providerCalls = 0, reservations = 0;
  const send = () => {
    providerCalls++;
    return Promise.resolve(new Response());
  };
  const db = {
    rpc: () => {
      reservations++;
      return Promise.reject(new LicenseError(429, "limit"));
    },
  };
  for (const file of [new File(["invalid"], "bad.wav"), fixture()]) {
    let caught = false;
    try {
      await transcribeFree(db, USER, file, new FormData(), "synthetic", send);
    } catch {
      caught = true;
    }
    assert(caught);
  }
  assert(providerCalls === 0 && reservations === 1);
});
Deno.test("timeout, provider5xx and invalid responses keep their reserved audio", async () => {
  for (
    const send of [
      () => Promise.reject(new Error("synthetic timeout")),
      () => Promise.resolve(new Response("unavailable", { status: 503 })),
      () => Promise.resolve(new Response("not JSON")),
      () => Promise.resolve(new Response(JSON.stringify({ text: 7 }))),
      () =>
        Promise.resolve(
          new Response(JSON.stringify({ text: "a".repeat(20001) })),
        ),
    ]
  ) {
    const db = recorder();
    const response = await transcribeFree(
      db,
      USER,
      fixture(),
      new FormData(),
      "synthetic",
      send,
    );
    assert(
      response.status === 502 && db.calls.length === 1 &&
        db.calls[0].name === "reserve_free_audio",
    );
  }
});
Deno.test("definite provider rejection releases audio but keeps the attempt, successful silence charges audio", async () => {
  for (const status of [400, 429]) {
    const db = recorder();
    const response = await transcribeFree(
      db,
      USER,
      fixture(),
      new FormData(),
      "synthetic",
      () => Promise.resolve(new Response("rejected", { status })),
    );
    assert(response.status === (status === 429 ? 429 : 502));
    assert(
      db.calls.length === 2 && db.calls[1].name === "finish_free_usage" &&
        db.calls[1].body.p_words === 0,
    );
  }
  const db = recorder();
  const response = await transcribeFree(
    db,
    USER,
    fixture(),
    new FormData(),
    "synthetic",
    () => Promise.resolve(new Response(JSON.stringify({ text: "" }))),
  );
  assert(
    response.status === 200 && db.calls.length === 2 &&
      db.calls[1].name === "finish_free_transcription" &&
      db.calls[1].body.p_words === 0,
  );
});
