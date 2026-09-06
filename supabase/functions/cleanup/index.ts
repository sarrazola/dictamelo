// Hosted cleanup uses a fixed economical model and server-owned token budgets.
import {
  cleanupBudget,
  finishPro,
  handler,
  jsonResponse,
  requireLicense,
  reservePro,
} from "../_shared/license.ts";
import { requireUser } from "../_shared/free.ts";
import { cleanFreeTranscript } from "../_shared/free_cleanup.ts";

const GROQ_URL = "https://api.groq.com/openai/v1/chat/completions";
const MODEL = "openai/gpt-oss-20b";

Deno.serve(handler(async (request, db) => {
  const freeUser = request.headers.has("x-license-key")
    ? null
    : await requireUser(request);
  const license = freeUser ? null : await requireLicense(request, db);
  const apiKey = Deno.env.get("GROQ_API_KEY");
  if (!apiKey) {
    return jsonResponse(
      { error: "The cleanup service is not configured." },
      503,
    );
  }
  const rawBody = await request.text();
  if (new TextEncoder().encode(rawBody).length > 100000) {
    return jsonResponse({ error: "The cleanup request is too large." }, 413);
  }
  let body: { system?: unknown; text?: unknown };
  try {
    body = JSON.parse(rawBody);
  } catch {
    return jsonResponse({ error: "Invalid cleanup request." }, 400);
  }
  if (freeUser) return await cleanFreeTranscript(db, freeUser.id, body, apiKey);
  if (
    !body || typeof body.system !== "string" || typeof body.text !== "string"
  ) {
    return jsonResponse({
      error: "Text and cleanup instructions are required.",
    }, 400);
  }
  const system = body.system.trim(), text = body.text.trim();
  const budget = cleanupBudget(system, text);
  const requestId = crypto.randomUUID();
  await reservePro(
    db,
    license!,
    requestId,
    "cleanup",
    0,
    budget.input,
    budget.output,
  );

  const response = await fetch(GROQ_URL, {
    method: "POST",
    headers: {
      Authorization: `Bearer ${apiKey}`,
      "Content-Type": "application/json",
    },
    body: JSON.stringify({
      model: MODEL,
      temperature: 0.2,
      reasoning_effort: "low",
      max_completion_tokens: budget.output,
      messages: [{ role: "system", content: system }, {
        role: "user",
        content: text,
      }],
    }),
    signal: AbortSignal.timeout(60000),
  });
  const raw = await response.text();
  if (!response.ok) {
    console.error("Cleanup provider HTTP status", response.status);
    // A definite rejection releases money quota, but still counts as an attempt.
    if (response.status >= 400 && response.status < 500) {
      await finishPro(db, license!, requestId);
    }
    return jsonResponse({
      error: "The cleanup service could not complete this request.",
    }, response.status === 429 ? 429 : 502);
  }
  const result = JSON.parse(raw) as {
    usage?: { prompt_tokens?: unknown; completion_tokens?: unknown };
    choices?: Array<{ finish_reason?: string }>;
  };
  const input = result.usage?.prompt_tokens,
    output = result.usage?.completion_tokens;
  if (
    !Number.isSafeInteger(input) || !Number.isSafeInteger(output) ||
    Number(input) < 0 || Number(output) < 0 || Number(input) > budget.input ||
    Number(output) > budget.output
  ) {
    // Keep the full conservative reservation when provider accounting is absent or inconsistent.
    return jsonResponse({
      error:
        "The cleanup service did not return valid usage. Please try later.",
    }, 502);
  }
  // completion_tokens includes reasoning tokens; do not add its details twice.
  await finishPro(db, license!, requestId, 0, Number(input), Number(output));
  if (result.choices?.[0]?.finish_reason === "length") {
    return jsonResponse({
      error:
        "This text is too long for one cleanup request. Use a shorter recording or your own API key.",
    }, 413);
  }
  return new Response(raw, {
    headers: {
      "Content-Type": "application/json",
      "Access-Control-Allow-Origin": "*",
    },
  });
}));
