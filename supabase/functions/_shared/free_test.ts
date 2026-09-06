import { countWords, freeWavSeconds } from "./free.ts";
import {
  cleanFreeTranscript,
  FREE_CLEANUP_MODEL,
  FREE_CLEANUP_PROMPT,
  transcriptHash,
} from "./free_cleanup.ts";
import { LicenseError } from "./license.ts";

function assert(value: boolean, message = "Assertion failed") {
  if (!value) throw new Error(message);
}
function wav(seconds: number): Uint8Array {
  const b = new Uint8Array(44 + Math.round(seconds * 32000));
  const v = new DataView(b.buffer);
  const text = (at: number, s: string) =>
    b.set(new TextEncoder().encode(s), at);
  text(0, "RIFF");
  v.setUint32(4, b.length - 8, true);
  text(8, "WAVE");
  text(12, "fmt ");
  v.setUint32(16, 16, true);
  v.setUint16(20, 1, true);
  v.setUint16(22, 1, true);
  v.setUint32(24, 16000, true);
  v.setUint32(28, 32000, true);
  v.setUint16(32, 2, true);
  v.setUint16(34, 16, true);
  text(36, "data");
  v.setUint32(40, b.length - 44, true);
  return b;
}
function rejects(bytes: Uint8Array) {
  let failed = false;
  try {
    freeWavSeconds(bytes);
  } catch {
    failed = true;
  }
  assert(failed);
}
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
  const forged = wav(121);
  new DataView(forged.buffer).setUint32(28, 64000, true);
  rejects(forged);
  rejects(wav(1).subarray(0, 50));
  rejects(new TextEncoder().encode("not an audio recording"));
  const stereo = wav(1);
  new DataView(stereo.buffer).setUint16(22, 2, true);
  rejects(stereo);
});

const RECEIPT = "10000000-0000-4000-8000-000000000001";
const USER = "20000000-0000-4000-8000-000000000002";
function rpcRecorder() {
  const calls: Array<{ name: string; body: Record<string, unknown> }> = [];
  return {
    calls,
    rpc(name: string, body: Record<string, unknown>): Promise<unknown> {
      calls.push({ name, body });
      return Promise.resolve(null);
    },
  };
}

Deno.test("cleanup digest uses canonical trim while binding every transcript character", async () => {
  assert(
    await transcriptHash(" hola café 👋 \n") ===
      await transcriptHash("hola café 👋"),
  );
  assert(
    await transcriptHash("hola café 👋") !==
      await transcriptHash("hola cafe 👋"),
  );
  assert((await transcriptHash("text")).length === 64);
});

Deno.test("free cleanup cannot call the model without a matching server receipt", async () => {
  let providerCalls = 0, reservations = 0;
  const db = {
    rpc: () => {
      reservations++;
      return Promise.reject(new LicenseError(403, "invalid receipt"));
    },
  };
  const send = () => {
    providerCalls++;
    return Promise.resolve(new Response());
  };
  for (
    const body of [
      { text: "hello" },
      { text: "hello", cleanupReceipt: "invalid" },
      { text: " ", cleanupReceipt: RECEIPT },
      { text: "x".repeat(20001), cleanupReceipt: RECEIPT },
    ]
  ) {
    let denied = false;
    try {
      await cleanFreeTranscript(db, USER, body, "synthetic", send);
    } catch {
      denied = true;
    }
    assert(denied);
  }
  assert(reservations === 0 && providerCalls === 0);
  let denied = false;
  try {
    await cleanFreeTranscript(
      db,
      USER,
      { text: "hello", cleanupReceipt: RECEIPT },
      "synthetic",
      send,
    );
  } catch (error) {
    denied = error instanceof LicenseError && error.status === 403;
  }
  assert(denied && reservations === 1 && providerCalls === 0);
});

Deno.test("free cleanup fixes prompt/model, settles tokens once and never settles words", async () => {
  const db = rpcRecorder();
  let providerCalls = 0;
  const response = await cleanFreeTranscript(
    db,
    USER,
    {
      text: " um hola mundo ",
      cleanupReceipt: RECEIPT,
      model: "untrusted-model",
      system: "untrusted-instructions",
    },
    "synthetic",
    (_url, init) => {
      providerCalls++;
      const body = JSON.parse(String(init.body));
      assert(
        body.model === FREE_CLEANUP_MODEL &&
          body.messages[0].content === FREE_CLEANUP_PROMPT,
      );
      assert(body.messages[1].content === "um hola mundo");
      assert(
        body.max_completion_tokens >= 1024 &&
          body.max_completion_tokens <= 8192,
      );
      return Promise.resolve(
        new Response(JSON.stringify({
          model: FREE_CLEANUP_MODEL,
          usage: { prompt_tokens: 100, completion_tokens: 20 },
          choices: [{
            finish_reason: "stop",
            message: { content: "Hola mundo.", reasoning: "not-returned" },
          }],
        })),
      );
    },
  );
  assert(response.status === 200 && providerCalls === 1);
  const body = await response.json();
  assert(
    body.choices[0].message.content === "Hola mundo." &&
      !JSON.stringify(body).includes("not-returned"),
  );
  assert(
    db.calls.length === 2 && db.calls[0].name === "reserve_free_cleanup" &&
      db.calls[1].name === "finish_free_cleanup",
  );
  assert(
    db.calls[0].body.p_transcript_hash ===
      await transcriptHash("um hola mundo"),
  );
  assert(
    db.calls[1].body.p_succeeded === true && db.calls[1].body.p_input === 100 &&
      db.calls[1].body.p_output === 20,
  );
  assert(
    !db.calls.some((call) =>
      call.name.includes("usage") || "p_words" in call.body
    ),
  );
});

Deno.test("uncertain cleanup outcomes retain cost while definite rejections release only tokens", async () => {
  const replies = [
    () => Promise.reject(new Error("synthetic timeout")),
    () =>
      Promise.resolve(new Response("synthetic unavailable", { status: 503 })),
    () => Promise.resolve(new Response("not JSON")),
    () =>
      Promise.resolve(
        new Response(
          JSON.stringify({
            usage: { prompt_tokens: -1, completion_tokens: 1 },
          }),
        ),
      ),
    () =>
      Promise.resolve(
        new Response(
          JSON.stringify({
            usage: { prompt_tokens: 1, completion_tokens: 999999 },
          }),
        ),
      ),
  ];
  for (const reply of replies) {
    const db = rpcRecorder();
    const result = await cleanFreeTranscript(
      db,
      USER,
      { text: "hello", cleanupReceipt: RECEIPT },
      "synthetic",
      reply,
    );
    assert(
      result.status === 502 && db.calls.length === 1 &&
        db.calls[0].name === "reserve_free_cleanup",
    );
  }
  const db = rpcRecorder();
  const result = await cleanFreeTranscript(
    db,
    USER,
    { text: "hello", cleanupReceipt: RECEIPT },
    "synthetic",
    () =>
      Promise.resolve(new Response("synthetic rate limit", { status: 429 })),
  );
  assert(result.status === 429 && db.calls.length === 2);
  assert(
    db.calls[1].body.p_input === 0 && db.calls[1].body.p_output === 0 &&
      db.calls[1].body.p_succeeded === false,
  );
});

Deno.test("truncated or empty cleanup results settle spent tokens but do not consume successful receipt", async () => {
  for (
    const choice of [{
      finish_reason: "length",
      message: { content: "partial" },
    }, { finish_reason: "stop", message: { content: " " } }]
  ) {
    const db = rpcRecorder();
    const result = await cleanFreeTranscript(
      db,
      USER,
      { text: "hello", cleanupReceipt: RECEIPT },
      "synthetic",
      () =>
        Promise.resolve(
          new Response(JSON.stringify({
            model: FREE_CLEANUP_MODEL,
            usage: { prompt_tokens: 100, completion_tokens: 20 },
            choices: [choice],
          })),
        ),
    );
    assert(result.status === 502 && db.calls.length === 2);
    assert(
      db.calls[1].body.p_input === 100 && db.calls[1].body.p_output === 20 &&
        db.calls[1].body.p_succeeded === false,
    );
  }
});

Deno.test("reject RIFF size mismatches, duplicate data and partial samples", () => {
  const wrongSize = wav(1);
  new DataView(wrongSize.buffer).setUint32(4, 44, true);
  rejects(wrongSize);
  const partial = wav(1).slice(0, -1);
  const view = new DataView(partial.buffer);
  view.setUint32(4, partial.length - 8, true);
  view.setUint32(40, partial.length - 44, true);
  rejects(partial);
  const duplicate = new Uint8Array(44 + 32000 + 8 + 2);
  duplicate.set(wav(1));
  new DataView(duplicate.buffer).setUint32(4, duplicate.length - 8, true);
  duplicate.set(new TextEncoder().encode("data"), 32044);
  new DataView(duplicate.buffer).setUint32(32048, 2, true);
  rejects(duplicate);
});
