-- Audio time is the public allowance. Historical word usage remains available to
-- released clients; only existing usage is estimated at 133 1/3 words per minute.
-- Requests after this migration use measured PCM duration, never client metadata.
alter table public.free_weekly_usage add column legacy_audio_seconds numeric not null default 0
  check (legacy_audio_seconds >= 0);
update public.free_weekly_usage set legacy_audio_seconds = words * 0.45;

create table public.free_audio_requests (
  request_id uuid primary key,
  user_id uuid not null,
  week_start date not null,
  state text not null default 'reserved' check (state in ('reserved', 'finished')),
  reserved_seconds numeric not null check (reserved_seconds > 0 and reserved_seconds <= 120.1),
  seconds numeric not null default 0 check (seconds >= 0),
  words integer not null default 0 check (words >= 0 and words <= 20000),
  reserved_until timestamptz not null default now() + interval '3 minutes',
  finished_at timestamptz,
  foreign key (user_id, week_start) references public.free_weekly_usage(user_id, week_start) on delete cascade
);
create index free_audio_requests_user_week_idx on public.free_audio_requests(user_id, week_start);
alter table public.free_audio_requests enable row level security;
revoke all on public.free_audio_requests from public, anon, authenticated;
grant all on public.free_audio_requests to service_role;

-- Account for an old function already processing an upload during deployment.
insert into public.free_audio_requests(request_id, user_id, week_start, reserved_seconds, reserved_until)
  select request_id, user_id, week_start, 120, coalesce(reserved_until, now())
  from public.free_weekly_usage where request_id is not null;

create or replace function public.free_usage(p_user uuid) returns jsonb
language sql stable security definer set search_path = '' as $$
  with current_week as (
    select date_trunc('week', now() at time zone 'UTC')::date as start
  ), totals as (
    select coalesce(sum(case when a.state = 'reserved' then a.reserved_seconds else a.seconds end), 0) as seconds
    from public.free_audio_requests a, current_week w where a.user_id = p_user and a.week_start = w.start
  )
  select jsonb_build_object('usedWords', coalesce(u.words, 0), 'limitWords', 2000,
    'usedSeconds', totals.seconds + coalesce(u.legacy_audio_seconds, 0), 'limitSeconds', 1800,
    'estimatedLegacySeconds', coalesce(u.legacy_audio_seconds, 0),
    'resetsAt', (w.start + interval '7 days') at time zone 'UTC')
  from current_week w cross join totals left join public.free_weekly_usage u on u.user_id = p_user and u.week_start = w.start;
$$;

create function public.reserve_free_audio(p_user uuid, p_request uuid, p_seconds numeric) returns void
language plpgsql security definer set search_path = '' as $$
declare w date := date_trunc('week', now() at time zone 'UTC')::date; u public.free_weekly_usage;
begin
  if p_user is null or p_request is null or p_seconds is null or p_seconds <= 0 or p_seconds > 120.1
    or p_seconds = 'NaN'::numeric then raise exception 'invalid_free_reservation'; end if;
  perform pg_catalog.pg_advisory_xact_lock(pg_catalog.hashtextextended(p_user::text, 7341));
  insert into public.free_weekly_usage(user_id, week_start) values(p_user, w) on conflict do nothing;
  select * into u from public.free_weekly_usage where user_id = p_user and week_start = w for update;
  -- The final accepted recording is delivered whole. One active request per account
  -- bounds the overage to a single two-minute recording, including at week rollover.
  if (public.free_usage(p_user)->>'usedSeconds')::numeric >= 1800 then raise exception 'weekly_audio_limit'; end if;
  if u.attempts >= 1000 then raise exception 'weekly_request_limit'; end if;
  if exists(select 1 from public.free_audio_requests where user_id = p_user and state = 'reserved' and reserved_until > now())
    or exists(select 1 from public.free_weekly_usage where user_id = p_user and reserved_until > now())
    then raise exception 'request_in_progress'; end if;
  insert into public.free_audio_requests(request_id,user_id,week_start,reserved_seconds) values(p_request,p_user,w,p_seconds);
  update public.free_weekly_usage set request_id = p_request, reserved_until = now() + interval '3 minutes', attempts = attempts + 1
    where user_id = p_user and week_start = w;
end;
$$;

-- Keep the legacy function signature for a staged deployment. Old handlers never
-- supplied duration, so their maximum two-minute recording is reserved safely.
create or replace function public.reserve_free_usage(p_user uuid, p_request uuid) returns void
language sql security definer set search_path = '' as $$ select public.reserve_free_audio(p_user,p_request,120); $$;

create function public.settle_free_audio(p_user uuid, p_request uuid, p_words integer, p_charge boolean) returns void
language plpgsql security definer set search_path = '' as $$
declare a public.free_audio_requests; charged numeric;
begin
  if p_user is null or p_request is null or p_words is null or p_words < 0 or p_words > 20000 or p_charge is null
    or (not p_charge and p_words <> 0) then raise exception 'invalid_free_transcription'; end if;
  perform pg_catalog.pg_advisory_xact_lock(pg_catalog.hashtextextended(p_user::text, 7341));
  select * into a from public.free_audio_requests where request_id = p_request and user_id = p_user for update;
  if not found then raise exception 'reservation_expired'; end if;
  charged := case when p_charge then a.reserved_seconds else 0 end;
  if a.state = 'finished' then
    if a.words <> p_words or a.seconds <> charged then raise exception 'reservation_completed'; end if;
    return;
  end if;
  update public.free_audio_requests set state = 'finished', seconds = charged, words = p_words, finished_at = now() where request_id = p_request;
  update public.free_weekly_usage set words = words + p_words,
    request_id = case when request_id = p_request then null else request_id end,
    reserved_until = case when request_id = p_request then now() + interval '2 seconds' else reserved_until end
    where user_id = p_user and week_start = a.week_start;
end;
$$;

-- Only an explicit provider rejection calls this compatibility refund path in the
-- current handler. Timeout/5xx/invalid responses retain their audio reservation.
create or replace function public.finish_free_usage(p_user uuid, p_request uuid, p_words integer) returns void
language sql security definer set search_path = '' as $$ select public.settle_free_audio(p_user,p_request,p_words,p_words > 0); $$;

create or replace function public.finish_free_transcription(p_user uuid, p_request uuid, p_words integer,
  p_transcript_hash text) returns uuid
language plpgsql security definer set search_path = '' as $$
declare r public.free_cleanup_receipts; w date;
begin
  if p_user is null or p_request is null or p_words is null or p_words < 0 or p_words > 20000
    or p_transcript_hash is null or p_transcript_hash !~ '^[a-f0-9]{64}$' then raise exception 'invalid_free_transcription'; end if;
  perform pg_catalog.pg_advisory_xact_lock(pg_catalog.hashtextextended(p_user::text, 7341));
  select * into r from public.free_cleanup_receipts where receipt_id = p_request;
  if found then
    if r.user_id <> p_user or r.transcript_hash <> p_transcript_hash or r.words <> p_words then raise exception 'invalid_cleanup_receipt'; end if;
    return r.receipt_id;
  end if;
  select week_start into w from public.free_audio_requests where user_id = p_user and request_id = p_request;
  if not found then raise exception 'reservation_expired'; end if;
  -- Successful silence still consumed audio processing; only text cleanup needs words.
  perform public.settle_free_audio(p_user,p_request,p_words,true);
  if p_words = 0 then return null; end if;
  insert into public.free_cleanup_receipts(receipt_id,user_id,week_start,transcript_hash,words) values(p_request,p_user,w,p_transcript_hash,p_words);
  return p_request;
end;
$$;

revoke all on function public.reserve_free_audio(uuid,uuid,numeric) from public,anon,authenticated;
revoke all on function public.settle_free_audio(uuid,uuid,integer,boolean) from public,anon,authenticated;
grant execute on function public.reserve_free_audio(uuid,uuid,numeric) to service_role;
grant execute on function public.settle_free_audio(uuid,uuid,integer,boolean) to service_role;

-- Three times the original paid allowance; preserve all existing licenses and usage.
create or replace function public.pro_usage(p_license uuid) returns jsonb
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
  select jsonb_build_object('usedSeconds', totals.seconds + legacy.seconds, 'limitSeconds', 648000,
    'inputTokens', totals.input, 'limitInputTokens', 9000000,
    'outputTokens', totals.output, 'limitOutputTokens', 6000000,
    'requests', totals.attempts + legacy.attempts, 'limitRequests', 36000)
  from totals, legacy;
$$;

create or replace function public.reserve_pro_usage(p_license uuid, p_request uuid, p_kind text,
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
  if (u->>'requests')::bigint >= 36000 then raise exception 'monthly_request_limit'; end if;
  if (u->>'usedSeconds')::numeric + p_seconds > 648000 then raise exception 'monthly_audio_limit'; end if;
  if (u->>'inputTokens')::bigint + p_input > 9000000
    or (u->>'outputTokens')::bigint + p_output > 6000000 then raise exception 'monthly_cleanup_limit'; end if;
  insert into public.pro_usage_requests(request_id, license_id, kind, reserved_seconds, reserved_input, reserved_output)
    values(p_request, p_license, p_kind, p_seconds, p_input, p_output);
end;
$$;


-- RLS already denied client access; remove legacy grants as defense in depth.
revoke all on public.licenses, public.usage_events from public, anon, authenticated;
grant all on public.licenses, public.usage_events to service_role;
