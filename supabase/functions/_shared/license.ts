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
/** Tope de audio por licencia cada 30 días. Generoso: 20 h son ~40 min diarios. */
const DEFAULT_MONTHLY_SECONDS = 20 * 60 * 60;

export class LicenseError extends Error {
  constructor(readonly status: number, message: string) {
    super(message);
  }
}

export interface License {
  id: string;
  status: string;
}

/** Acceso mínimo a PostgREST con el rol de servicio. */
export class Db {
  private readonly base: string;
  private readonly headers: Record<string, string>;

  constructor() {
    const url = Deno.env.get("SUPABASE_URL");
    const key = Deno.env.get("SUPABASE_SERVICE_ROLE_KEY");
    if (!url || !key) throw new LicenseError(500, "El servidor no está configurado");
    this.base = `${url}/rest/v1`;
    this.headers = {
      apikey: key,
      Authorization: `Bearer ${key}`,
      "Content-Type": "application/json",
    };
  }

  private async request(path: string, init: RequestInit): Promise<unknown> {
    const response = await fetch(`${this.base}${path}`, { ...init, headers: { ...this.headers, ...init.headers } });
    const text = await response.text();
    if (!response.ok) {
      if (text.includes("weekly_word_limit")) throw new LicenseError(429, "You have used your 2,000 free words this week. Your allowance renews on Monday at 00:00 UTC. Upgrade to Pro or use your own API key.");
      if (text.includes("weekly_request_limit")) throw new LicenseError(429, "This week's free request limit has been reached.");
      if (text.includes("request_in_progress")) throw new LicenseError(409, "Another recording is being processed. Please try again in a few seconds.");
      console.error("PostgREST", response.status, text.slice(0, 300));
      throw new LicenseError(500, "Error de base de datos");
    }
    return text ? JSON.parse(text) : null;
  }

  async rpc(name: string, body: Record<string, unknown>): Promise<unknown> {
    return await this.request(`/rpc/${name}`, { method: "POST", body: JSON.stringify(body) });
  }

  async findLicense(keyHash: string): Promise<{ id: string; status: string; checked_at: string } | null> {
    const rows = (await this.request(
      `/licenses?key_hash=eq.${encodeURIComponent(keyHash)}&select=id,status,checked_at&limit=1`,
      { method: "GET" },
    )) as Array<{ id: string; status: string; checked_at: string }>;
    return rows?.[0] ?? null;
  }

  async upsertLicense(keyHash: string, instanceId: string | null, status: string): Promise<License> {
    const rows = (await this.request("/licenses?on_conflict=key_hash", {
      method: "POST",
      headers: { Prefer: "resolution=merge-duplicates,return=representation" },
      body: JSON.stringify([{
        key_hash: keyHash,
        instance_id: instanceId,
        status,
        checked_at: new Date().toISOString(),
      }]),
    })) as Array<License>;
    if (!rows?.[0]) throw new LicenseError(500, "No se pudo registrar la licencia");
    return { id: rows[0].id, status: rows[0].status };
  }

  async usageLast30Days(licenseId: string): Promise<number> {
    const value = await this.request("/rpc/usage_last_30_days", {
      method: "POST",
      body: JSON.stringify({ p_license: licenseId }),
    });
    return Number(value ?? 0);
  }

  async insertUsage(licenseId: string, seconds: number, kind: string): Promise<void> {
    await this.request("/usage_events", {
      method: "POST",
      headers: { Prefer: "return=minimal" },
      body: JSON.stringify([{ license_id: licenseId, seconds, kind }]),
    });
  }
}

/** SHA-256 en hexadecimal. La clave en claro nunca se guarda. */
async function hashKey(key: string): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(key));
  return Array.from(new Uint8Array(digest)).map((b) => b.toString(16).padStart(2, "0")).join("");
}

interface LemonResponse {
  valid?: boolean;
  error?: string;
  license_key?: { status?: string };
}

async function validateWithLemonSqueezy(key: string, instanceId: string | null): Promise<string> {
  const body: Record<string, string> = { license_key: key };
  if (instanceId) body.instance_id = instanceId;
  const response = await fetch("https://api.lemonsqueezy.com/v1/licenses/validate", {
    method: "POST",
    headers: { "Content-Type": "application/json", Accept: "application/json" },
    body: JSON.stringify(body),
  });
  if (response.status >= 500) {
    throw new LicenseError(503, "No se pudo comprobar la licencia; inténtalo en un momento");
  }
  const data = (await response.json()) as LemonResponse;
  if (data.valid) return data.license_key?.status ?? "active";
  return data.license_key?.status ?? "invalid";
}

/**
 * Valida la licencia de la petición. Devuelve la fila o lanza `LicenseError`.
 * Usa la caché mientras sea reciente para que un dictado no espere a Lemon Squeezy.
 */
export async function requireLicense(request: Request, db: Db): Promise<License> {
  const key = request.headers.get("x-license-key")?.trim();
  const instanceId = request.headers.get("x-license-instance")?.trim() || null;
  if (!key) throw new LicenseError(401, "Falta la licencia");

  const keyHash = await hashKey(key);
  const cached = await db.findLicense(keyHash);
  const fresh = cached && Date.now() - new Date(cached.checked_at).getTime() < REVALIDATE_AFTER_MS;
  if (fresh && cached.status === "active") return { id: cached.id, status: cached.status };

  const status = await validateWithLemonSqueezy(key, instanceId);
  if (status !== "active") {
    // No se guarda nada de las claves que no sirven: si se cacheara cada intento fallido,
    // cualquiera podría llenar la tabla probando claves al azar.
    throw new LicenseError(403, "La licencia no está activa. Revisa tu suscripción.");
  }
  return await db.upsertLicense(keyHash, instanceId, status);
}

/** Corta si la licencia superó el tope de los últimos 30 días. */
export async function requireQuota(license: License, db: Db): Promise<void> {
  const limit = Number(Deno.env.get("MONTHLY_SECONDS") ?? DEFAULT_MONTHLY_SECONDS);
  if (!Number.isFinite(limit) || limit <= 0) return;
  try {
    if (await db.usageLast30Days(license.id) >= limit) {
      throw new LicenseError(429, "Alcanzaste el máximo de este mes. Escríbenos si necesitas más.");
    }
  } catch (e) {
    // Un fallo al consultar el consumo no debe castigar a quien sí pagó.
    if (e instanceof LicenseError && e.status === 429) throw e;
    console.error("No se pudo leer el consumo:", e);
  }
}

/** Registra el consumo. Nunca guarda audio ni texto, solo la duración. */
export async function recordUsage(
  license: License,
  db: Db,
  seconds: number,
  kind: "transcribe" | "cleanup",
): Promise<void> {
  if (!Number.isFinite(seconds) || seconds <= 0) return;
  try {
    await db.insertUsage(license.id, Math.round(seconds * 100) / 100, kind);
  } catch (e) {
    console.error("No se pudo registrar el consumo:", e);
  }
}

export const CORS_HEADERS: Record<string, string> = {
  "Access-Control-Allow-Origin": "*",
  "Access-Control-Allow-Headers": "authorization, content-type, x-license-key, x-license-instance",
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
    if (request.method === "OPTIONS") return new Response("ok", { headers: CORS_HEADERS });
    if (request.method !== "POST") {
      await discardBody(request);
      return jsonResponse({ error: "Método no permitido" }, 405);
    }
    try {
      return await fn(request, new Db());
    } catch (e) {
      await discardBody(request);
      if (e instanceof LicenseError) return jsonResponse({ error: e.message }, e.status);
      // El detalle va al registro del servidor; al cliente solo un mensaje neutro.
      console.error("Fallo no controlado:", e);
      return jsonResponse({ error: "Error interno" }, 500);
    }
  };
}
