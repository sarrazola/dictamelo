import { handler, jsonResponse } from "../_shared/license.ts";
import { requireUser } from "../_shared/free.ts";

Deno.serve(handler(async (request, db) => {
  const user = await requireUser(request);
  return jsonResponse({ email: user.email, ...await db.rpc("free_usage", { p_user: user.id }) as object });
}));
