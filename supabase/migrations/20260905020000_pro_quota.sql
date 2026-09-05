-- Paid quotas are server-owned. Reservations prevent concurrent calls from
-- spending the same allowance; abandoned requests retain their reserved cost.
alter table public.licenses add column lemon_store_id bigint;
alter table public.licenses add column lemon_product_id bigint;
alter table public.licenses add column lemon_variant_id bigint;

create table public.pro_usage_requests (
  request_id uuid primary key,
  license_id uuid not null references public.licenses(id) on delete cascade,
  kind text not null check (kind in ('transcribe', 'cleanup')),
  state text not null default 'reserved' check (state in ('reserved', 'finished')),
  reserved_seconds numeric not null default 0 check (reserved_seconds >= 0),
  reserved_input bigint not null default 0 check (reserved_input >= 0),
  reserved_output bigint not null default 0 check (reserved_output >= 0),
  seconds numeric not null default 0 check (seconds >= 0),
  input_tokens bigint not null default 0 check (input_tokens >= 0),
  output_tokens bigint not null default 0 check (output_tokens >= 0),
  created_at timestamptz not null default now(),
  reserved_until timestamptz not null default now() + interval '3 minutes',
  finished_at timestamptz
);
create index pro_usage_requests_license_created_idx
  on public.pro_usage_requests(license_id, created_at desc);
alter table public.pro_usage_requests enable row level security;
revoke all on public.pro_usage_requests from public, anon, authenticated;
grant all on public.pro_usage_requests to service_role;

create function public.pro_usage(p_license uuid) returns jsonb
language sql stable security definer set search_path = '' as $$
  with totals as (
    select coalesce(sum(case when state = 'reserved' then reserved_seconds else seconds end), 0) as seconds,
      coalesce(sum(case when state = 'reserved' then reserved_input else input_tokens end), 0) as input,
      coalesce(sum(case when state = 'reserved' then reserved_output else output_tokens end), 0) as output,
      count(*) as attempts
    from public.pro_usage_requests where license_id = p_license and created_at > now() - interval '30 days'
  ), legacy as (
    select coalesce(sum(seconds) filter (where kind = 'transcribe'), 0) as seconds,
      count(*) as attempts from public.usage_events
    where license_id = p_license and created_at > now() - interval '30 days'
  )
  select jsonb_build_object('usedSeconds', totals.seconds + legacy.seconds, 'limitSeconds', 216000,
    'inputTokens', totals.input, 'limitInputTokens', 3000000,
    'outputTokens', totals.output, 'limitOutputTokens', 2000000,
    'requests', totals.attempts + legacy.attempts, 'limitRequests', 12000)
  from totals, legacy;
$$;

create function public.reserve_pro_usage(p_license uuid, p_request uuid, p_kind text,
  p_seconds numeric, p_input bigint, p_output bigint) returns void
language plpgsql security definer set search_path = '' as $$
declare u jsonb;
begin
  if p_license is null or p_request is null or p_kind is null or p_seconds is null or p_input is null or p_output is null
    or p_kind not in ('transcribe', 'cleanup') or p_seconds < 0 or p_seconds > 600.1
    or p_input < 0 or p_input > 100000 or p_output < 0 or p_output > 8192
    or (p_kind = 'transcribe' and (p_seconds < 10 or p_input <> 0 or p_output <> 0))
    or (p_kind = 'cleanup' and (p_seconds <> 0 or p_input = 0 or p_output = 0))
  then raise exception 'invalid_pro_reservation'; end if;
  perform 1 from public.licenses where id = p_license for update;
  if not found then raise exception 'unknown_license'; end if;
  if exists(select 1 from public.pro_usage_requests where license_id = p_license
    and state = 'reserved' and reserved_until > now())
  then raise exception 'request_in_progress'; end if;
  u := public.pro_usage(p_license);
  if (u->>'requests')::bigint >= 12000 then raise exception 'monthly_request_limit'; end if;
  if (u->>'usedSeconds')::numeric + p_seconds > 216000 then raise exception 'monthly_audio_limit'; end if;
  if (u->>'inputTokens')::bigint + p_input > 3000000
    or (u->>'outputTokens')::bigint + p_output > 2000000 then raise exception 'monthly_cleanup_limit'; end if;
  insert into public.pro_usage_requests(request_id, license_id, kind, reserved_seconds, reserved_input, reserved_output)
    values(p_request, p_license, p_kind, p_seconds, p_input, p_output);
end;
$$;

create function public.finish_pro_usage(p_license uuid, p_request uuid,
  p_seconds numeric, p_input bigint, p_output bigint) returns void
language plpgsql security definer set search_path = '' as $$
declare r public.pro_usage_requests;
begin
  if p_license is null or p_request is null or p_seconds is null or p_input is null or p_output is null
    or p_seconds < 0 or p_seconds > 10000000 or p_input < 0 or p_output < 0
    then raise exception 'invalid_pro_usage'; end if;
  perform 1 from public.licenses where id = p_license for update;
  select * into r from public.pro_usage_requests where license_id = p_license and request_id = p_request for update;
  if not found then raise exception 'reservation_expired'; end if;
  if r.state = 'finished' then raise exception 'reservation_completed'; end if;
  if (r.kind = 'transcribe' and (p_input <> 0 or p_output <> 0))
    or (r.kind = 'cleanup' and (p_seconds <> 0 or p_input > r.reserved_input or p_output > r.reserved_output))
    then raise exception 'invalid_pro_usage'; end if;
  -- Legacy compressed uploads can exceed the preflight estimate. Record their
  -- real duration even when the handler rejects the result; future calls stop.
  update public.pro_usage_requests set state = 'finished', seconds = p_seconds,
    input_tokens = p_input, output_tokens = p_output, finished_at = now()
    where request_id = p_request;
end;
$$;

revoke all on function public.pro_usage(uuid) from public, anon, authenticated;
revoke all on function public.reserve_pro_usage(uuid, uuid, text, numeric, bigint, bigint) from public, anon, authenticated;
revoke all on function public.finish_pro_usage(uuid, uuid, numeric, bigint, bigint) from public, anon, authenticated;
grant execute on function public.pro_usage(uuid) to service_role;
grant execute on function public.reserve_pro_usage(uuid, uuid, text, numeric, bigint, bigint) to service_role;
grant execute on function public.finish_pro_usage(uuid, uuid, numeric, bigint, bigint) to service_role;
