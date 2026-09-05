// Comprobación de licencia y tope de consumo, compartida por las funciones de borde.
//
// La app manda su clave de licencia en la cabecera `x-license-key`. Aquí se valida contra
// Lemon Squeezy (con caché en la base para no llamar en cada dictado) y se controla un tope
// mensual generoso que solo existe para frenar abusos.
//
// Se habla con PostgREST por HTTP en vez de usar el SDK: son cuatro consultas, y así la función
// no arrastra dependencias ni paga arranque en frío.

/** Cada cuánto se revalida una licencia contra Lemon Squeezy. */
const REVALIDATE_AFTER_MS = 60 * 60 * 1000; // 1 hora
/** Server-enforced rolling allowance; source of truth is reserve_pro_usage. */
export const PRO_AUDIO_SECONDS = 60 * 60 * 60;
export const PRO_REQUEST_SECONDS = 600;

export class LicenseError extends Error {
  constructor(readonly status: number, message: string) {
    super(message);
  }
}

export interface License {
  id: string;
  status: string;
}

interface CachedLicense extends License {
  checked_at: string;
  instance_id: string | null;
  lemon_store_id: string | number | null;
  lemon_product_id: string | number | null;
  lemon_variant_id: string | number | null;
}

export interface LemonOwnership {
  storeId: string;
  productId: string;
  variantIds: string[];
}

export function lemonOwnership(
  values: Record<string, string | undefined>,
): LemonOwnership {
  const storeId = values.LEMON_STORE_ID?.trim() ?? "";
  const productId = values.LEMON_PRODUCT_ID?.trim() ?? "";
  const variantIds = (values.LEMON_VARIANT_IDS ?? "").split(",").map((s) =>
    s.trim()
  ).filter(Boolean);
  if (
    ![storeId, productId, ...variantIds].every((id) =>
      /^[1-9][0-9]*$/.test(id)
    ) || !variantIds.length
  ) {
    throw new LicenseError(
      503,
      "Hosted Pro is not configured. Please try again later.",
    );
  }
  return { storeId, productId, variantIds };
}

export function matchesOwnership(
  meta:
    | { store_id?: unknown; product_id?: unknown; variant_id?: unknown }
    | undefined,
  expected: LemonOwnership,
): boolean {
  return !!meta && String(meta.store_id) === expected.storeId &&
    String(meta.product_id) === expected.productId &&
    expected.variantIds.includes(String(meta.variant_id));
}

/** Acceso mínimo a PostgREST con el rol de servicio. */
export class Db {
  private readonly base: string;
  private readonly headers: Record<string, string>;

  constructor() {
    const url = Deno.env.get("SUPABASE_URL");
    const key = Deno.env.get("SUPABASE_SERVICE_ROLE_KEY");
    if (!url || !key) {
      throw new LicenseError(500, "El servidor no está configurado");
    }
    this.base = `${url}/rest/v1`;
    this.headers = {
      apikey: key,
      Authorization: `Bearer ${key}`,
      "Content-Type": "application/json",
    };
  }

  private async request(path: string, init: RequestInit): Promise<unknown> {
    const response = await fetch(`${this.base}${path}`, {
      ...init,
      headers: { ...this.headers, ...init.headers },
    });
    const text = await response.text();
    if (!response.ok) {
      if (text.includes("weekly_word_limit")) {
        throw new LicenseError(
          429,
          "You have used your 2,000 free words this week. Your allowance renews on Monday at 00:00 UTC. Upgrade to Pro or use your own API key.",
        );
      }
      if (text.includes("weekly_request_limit")) {
        throw new LicenseError(
          429,
          "This week's free request limit has been reached.",
        );
      }
      if (text.includes("monthly_audio_limit")) {
        throw new LicenseError(
          429,
          "You have used your 60 hours of Pro audio in the last 30 days, or this recording exceeds your remaining allowance. Use your own API key or wait for earlier usage to leave the rolling window.",
        );
      }
      if (text.includes("monthly_cleanup_limit")) {
        throw new LicenseError(
          429,
          "Your Pro text-cleanup allowance has been reached. Dictation remains available within your audio allowance.",
        );
      }
      if (text.includes("monthly_request_limit")) {
        throw new LicenseError(
          429,
          "Your 12,000 Pro requests in the last 30 days have been used. Use your own API key or try later.",
        );
      }
      if (text.includes("request_in_progress")) {
        throw new LicenseError(
          409,
          "Another recording is being processed. Please try again in a few seconds.",
        );
      }
      console.error("PostgREST", response.status, text.slice(0, 300));
      throw new LicenseError(500, "Error de base de datos");
    }
    return text ? JSON.parse(text) : null;
  }

  async rpc(name: string, body: Record<string, unknown>): Promise<unknown> {
    return await this.request(`/rpc/${name}`, {
      method: "POST",
      body: JSON.stringify(body),
    });
  }

  async findLicense(keyHash: string): Promise<CachedLicense | null> {
    const rows = (await this.request(
      `/licenses?key_hash=eq.${
        encodeURIComponent(keyHash)
      }&select=id,status,checked_at,instance_id,lemon_store_id,lemon_product_id,lemon_variant_id&limit=1`,
      { method: "GET" },
    )) as Array<CachedLicense>;
    return rows?.[0] ?? null;
  }

  async upsertLicense(
    keyHash: string,
    instanceId: string | null,
    status: string,
    meta: { store_id: unknown; product_id: unknown; variant_id: unknown },
  ): Promise<License> {
    const rows = (await this.request("/licenses?on_conflict=key_hash", {
      method: "POST",
      headers: { Prefer: "resolution=merge-duplicates,return=representation" },
      body: JSON.stringify([{
        key_hash: keyHash,
        instance_id: instanceId,
        status,
        lemon_store_id: meta.store_id,
        lemon_product_id: meta.product_id,
        lemon_variant_id: meta.variant_id,
        checked_at: new Date().toISOString(),
      }]),
    })) as Array<License>;
    if (!rows?.[0]) {
      throw new LicenseError(500, "No se pudo registrar la licencia");
    }
    return { id: rows[0].id, status: rows[0].status };
  }
}

/** SHA-256 en hexadecimal. La clave en claro nunca se guarda. */
async function hashKey(key: string): Promise<string> {
  const digest = await crypto.subtle.digest(
    "SHA-256",
    new TextEncoder().encode(key),
  );
  return Array.from(new Uint8Array(digest)).map((b) =>
    b.toString(16).padStart(2, "0")
  ).join("");
}

interface LemonResponse {
  valid?: boolean;
  license_key?: { status?: string };
  meta?: { store_id: unknown; product_id: unknown; variant_id: unknown };
}

async function validateWithLemonSqueezy(
  key: string,
  instanceId: string | null,
  expected: LemonOwnership,
): Promise<LemonResponse> {
  const body: Record<string, string> = { license_key: key };
  if (instanceId) body.instance_id = instanceId;
  const response = await fetch(
    "https://api.lemonsqueezy.com/v1/licenses/validate",
    {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json",
      },
      body: JSON.stringify(body),
      signal: AbortSignal.timeout(10000),
    },
  );
  if (response.status >= 500 || response.status === 429) {
    throw new LicenseError(
      503,
      "License verification is temporarily unavailable. Please try again.",
    );
  }
  const data = (await response.json()) as LemonResponse;
  if (
    !response.ok || !data.valid || data.license_key?.status !== "active" ||
    !matchesOwnership(data.meta, expected)
  ) {
    throw new LicenseError(
      403,
      "This license is not an active Dictámelo Pro license. Check your subscription.",
    );
  }
  return data;
}

/** Both new validations and cached entries must belong to this exact product. */
export async function requireLicense(
  request: Request,
  db: Db,
): Promise<License> {
  const key = request.headers.get("x-license-key")?.trim();
  const instanceId = request.headers.get("x-license-instance")?.trim() || null;
  if (!key) throw new LicenseError(401, "A Pro license is required.");
  const expected = lemonOwnership({
    LEMON_STORE_ID: Deno.env.get("LEMON_STORE_ID"),
    LEMON_PRODUCT_ID: Deno.env.get("LEMON_PRODUCT_ID"),
    LEMON_VARIANT_IDS: Deno.env.get("LEMON_VARIANT_IDS"),
  });
  const keyHash = await hashKey(key);
  const cached = await db.findLicense(keyHash);
  const fresh = cached &&
    Date.now() - new Date(cached.checked_at).getTime() < REVALIDATE_AFTER_MS;
  if (
    fresh && cached.status === "active" && cached.instance_id === instanceId &&
    matchesOwnership({
      store_id: cached.lemon_store_id,
      product_id: cached.lemon_product_id,
      variant_id: cached.lemon_variant_id,
    }, expected)
  ) return { id: cached.id, status: cached.status };

  const data = await validateWithLemonSqueezy(key, instanceId, expected);
  return await db.upsertLicense(keyHash, instanceId, "active", data.meta!);
}

export async function reservePro(
  db: Db,
  license: License,
  requestId: string,
  kind: "transcribe" | "cleanup",
  seconds = 0,
  input = 0,
  output = 0,
): Promise<void> {
  await db.rpc("reserve_pro_usage", {
    p_license: license.id,
    p_request: requestId,
    p_kind: kind,
    p_seconds: seconds,
    p_input: input,
    p_output: output,
  });
}

export async function finishPro(
  db: Db,
  license: License,
  requestId: string,
  seconds = 0,
  input = 0,
  output = 0,
): Promise<void> {
  await db.rpc("finish_pro_usage", {
    p_license: license.id,
    p_request: requestId,
    p_seconds: seconds,
    p_input: input,
    p_output: output,
  });
}

/** A strict PCM parser for current clients; null keeps older compressed uploads compatible. */
export function proWavSeconds(bytes: Uint8Array): number | null {
  const tag = (at: number) =>
    new TextDecoder().decode(bytes.subarray(at, at + 4));
  if (tag(0) !== "RIFF") return null;
  const invalid = () => new LicenseError(400, "Invalid PCM WAV recording.");
  if (bytes.length < 44 || tag(8) !== "WAVE") throw invalid();
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  if (view.getUint32(4, true) + 8 !== bytes.length) throw invalid();
  let rate = 0, size = 0, format = false;
  for (let at = 12; at + 8 <= bytes.length;) {
    const length = view.getUint32(at + 4, true);
    if (at + 8 + length > bytes.length) throw invalid();
    if (tag(at) === "fmt ") {
      if (format || length < 16) throw invalid();
      format = true;
      const channels = view.getUint16(at + 10, true),
        hz = view.getUint32(at + 12, true);
      const byteRate = view.getUint32(at + 16, true),
        align = view.getUint16(at + 20, true),
        bits = view.getUint16(at + 22, true);
      if (
        view.getUint16(at + 8, true) !== 1 || channels < 1 || channels > 2 ||
        hz < 8000 || hz > 192000 || bits !== 16 || align !== channels * 2 ||
        byteRate !== hz * align
      ) throw invalid();
      rate = byteRate;
    }
    if (tag(at) === "data") size += length;
    at += 8 + length + (length % 2);
  }
  if (!rate || !size || size % 2) throw invalid();
  const seconds = size / rate;
  if (seconds > PRO_REQUEST_SECONDS + 0.1) {
    throw new LicenseError(
      413,
      "Pro recordings can be up to ten minutes. Split this file or use your own API key.",
    );
  }
  return seconds;
}

/** Byte counting is a conservative token reservation, not a tokenizer estimate. */
export function cleanupBudget(
  system: string,
  text: string,
): { input: number; output: number } {
  if (!system.trim() || !text.trim()) {
    throw new LicenseError(400, "Text and cleanup instructions are required.");
  }
  if (system.length > 2000 || text.length > 20000) {
    throw new LicenseError(
      413,
      "The text or cleanup instructions are too long.",
    );
  }
  const encoded = new TextEncoder().encode(
    JSON.stringify([{ role: "system", content: system }, {
      role: "user",
      content: text,
    }]),
  );
  const input = encoded.length + 256;
  if (input > 100000) {
    throw new LicenseError(413, "The cleanup request is too large.");
  }
  const output = Math.min(
    8192,
    Math.max(1024, new TextEncoder().encode(text).length * 2),
  );
  return { input, output };
}

export const CORS_HEADERS: Record<string, string> = {
  "Access-Control-Allow-Origin": "*",
  "Access-Control-Allow-Headers":
    "authorization, content-type, x-license-key, x-license-instance",
  "Access-Control-Allow-Methods": "POST, OPTIONS",
};

export function jsonResponse(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { ...CORS_HEADERS, "Content-Type": "application/json" },
  });
}

/**
 * Lee y tira el cuerpo que no se llegó a usar.
 *
 * Hay que consumirlo, no basta con cancelarlo: si se responde a una petición con audio adjunto
 * sin leer el flujo, el cliente sigue subiendo, nadie lee, y la conexión se queda colgada hasta
 * que el proxy la corta con un 504. Absorber los bytes cuesta ancho de banda, pero el rechazo
 * ocurre antes de tocar al proveedor, que es lo que de verdad cuesta dinero.
 */
async function discardBody(request: Request): Promise<void> {
  try {
    if (request.body && !request.bodyUsed) await request.arrayBuffer();
  } catch {
    // Ya estaba consumido, o el cliente cortó: no hay nada que hacer.
  }
}

/** Envuelve un manejador con CORS y traducción de errores a respuestas limpias. */
export function handler(fn: (request: Request, db: Db) => Promise<Response>) {
  return async (request: Request): Promise<Response> => {
    if (request.method === "OPTIONS") {
      return new Response("ok", { headers: CORS_HEADERS });
    }
    if (request.method !== "POST") {
      await discardBody(request);
      return jsonResponse({ error: "Método no permitido" }, 405);
    }
    try {
      return await fn(request, new Db());
    } catch (e) {
      await discardBody(request);
      if (e instanceof LicenseError) {
        return jsonResponse({ error: e.message }, e.status);
      }
      // El detalle va al registro del servidor; al cliente solo un mensaje neutro.
      console.error("Fallo no controlado:", e);
      return jsonResponse({ error: "Error interno" }, 500);
    }
  };
}
