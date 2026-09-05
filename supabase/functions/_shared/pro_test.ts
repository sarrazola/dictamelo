import {
  cleanupBudget,
  lemonOwnership,
  matchesOwnership,
  proWavSeconds,
} from "./license.ts";

function assert(value: boolean, message = "Assertion failed") {
  if (!value) throw new Error(message);
}
function rejects(fn: () => unknown) {
  let failed = false;
  try {
    fn();
  } catch {
    failed = true;
  }
  assert(failed);
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

Deno.test("Pro validates the actual WAV duration and structural byte rate", () => {
  assert(proWavSeconds(wav(600)) === 600);
  rejects(() => proWavSeconds(wav(601)));
  const forged = wav(601);
  new DataView(forged.buffer).setUint32(28, 64000, true);
  rejects(() => proWavSeconds(forged));
  rejects(() => proWavSeconds(wav(1).subarray(0, 50)));
  assert(
    proWavSeconds(new TextEncoder().encode("ID3 legacy compressed upload")) ===
      null,
  );
});

Deno.test("cleanup reserves UTF-8 bytes and bounds arbitrary instructions and completion", () => {
  const b = cleanupBudget("Remove fillers", "こんにちは 👋");
  assert(b.input > new TextEncoder().encode("こんにちは 👋").length);
  assert(b.output === 1024);
  assert(cleanupBudget("Clean", "a".repeat(20000)).output === 8192);
  rejects(() => cleanupBudget("a".repeat(2001), "short"));
  rejects(() => cleanupBudget("Clean", "a".repeat(20001)));
  rejects(() => cleanupBudget(" ", "short"));
});

Deno.test("license ownership fails closed and rejects unrelated valid product IDs", () => {
  const expected = lemonOwnership({
    LEMON_STORE_ID: "12",
    LEMON_PRODUCT_ID: "34",
    LEMON_VARIANT_IDS: "56, 78",
  });
  assert(
    matchesOwnership(
      { store_id: 12, product_id: "34", variant_id: 78 },
      expected,
    ),
  );
  assert(
    !matchesOwnership(
      { store_id: 12, product_id: 999, variant_id: 78 },
      expected,
    ),
  );
  assert(
    !matchesOwnership(
      { store_id: 13, product_id: 34, variant_id: 78 },
      expected,
    ),
  );
  assert(
    !matchesOwnership(
      { store_id: 12, product_id: 34, variant_id: 99 },
      expected,
    ),
  );
  assert(!matchesOwnership(undefined, expected));
  rejects(() => lemonOwnership({}));
  rejects(() =>
    lemonOwnership({
      LEMON_STORE_ID: "12",
      LEMON_PRODUCT_ID: "34",
      LEMON_VARIANT_IDS: "",
    })
  );
  rejects(() =>
    lemonOwnership({
      LEMON_STORE_ID: "12",
      LEMON_PRODUCT_ID: "34x",
      LEMON_VARIANT_IDS: "56",
    })
  );
});
