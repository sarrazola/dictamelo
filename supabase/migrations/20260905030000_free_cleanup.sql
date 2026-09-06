-- Free cleanup belongs to a completed, metered transcription. Only a digest is retained;
-- neither the transcript nor the model's answer is stored in these tables.
create table public.free_cleanup_receipts (
  receipt_id uuid primary key,
  user_id uuid not null references auth.users(id) on delete cascade,
  week_start date not null,
  transcript_hash text not null check (transcript_hash ~ '^[a-f0-9]{64}$'),
  words integer not null check (words > 0 and words <= 20000),
  created_at timestamptz not null default now(),
  expires_at timestamptz not null default now() + interval '24 hours',
  completed_at timestamptz,
  foreign key (user_id, week_start) references public.free_weekly_usage(user_id, week_start) on delete cascade
);
create index free_cleanup_receipts_user_week_idx on public.free_cleanup_receipts(user_id, week_start);

create table public.free_cleanup_attempts (
  request_id uuid primary key,
  receipt_id uuid not null references public.free_cleanup_receipts(receipt_id) on delete cascade,
  state text not null default 'reserved' check (state in ('reserved', 'finished')),
  reserved_input integer not null check (reserved_input > 0 and reserved_input <= 100000),
  reserved_output integer not null check (reserved_output > 0 and reserved_output <= 8192),
  input_tokens integer not null default 0 check (input_tokens >= 0),
  output_tokens integer not null default 0 check (output_tokens >= 0),
  succeeded boolean not null default false,
  reserved_until timestamptz not null default now() + interval '3 minutes',
  finished_at timestamptz
);
create index free_cleanup_attempts_receipt_idx on public.free_cleanup_attempts(receipt_id);
alter table public.free_cleanup_receipts enable row level security;
alter table public.free_cleanup_attempts enable row level security;
revoke all on public.free_cleanup_receipts, public.free_cleanup_attempts from public, anon, authenticated;
grant all on public.free_cleanup_receipts, public.free_cleanup_attempts to service_role;

-- The old finish_free_usage signature remains available to existing clients. The new
-- function settles words and creates the receipt in one transaction, never two charges.
create function public.finish_free_transcription(p_user uuid, p_request uuid, p_words integer,
  p_transcript_hash text) returns uuid
language plpgsql security definer set search_path = '' as $$
declare r public.free_cleanup_receipts; w date;
begin
  if p_user is null or p_request is null or p_words is null or p_words < 0 or p_words > 20000
    or p_transcript_hash is null or p_transcript_hash !~ '^[a-f0-9]{64}$'
  then raise exception 'invalid_free_transcription'; end if;
  -- One account lock serializes cleanup claims and completions even across a UTC week boundary.
  perform pg_catalog.pg_advisory_xact_lock(pg_catalog.hashtextextended(p_user::text, 7341));
  select * into r from public.free_cleanup_receipts where receipt_id = p_request;
  if found then
    if r.user_id <> p_user or r.transcript_hash <> p_transcript_hash or r.words <> p_words
    then raise exception 'invalid_cleanup_receipt'; end if;
    return r.receipt_id;
  end if;
  select week_start into w from public.free_weekly_usage
    where user_id = p_user and request_id = p_request for update;
  if not found then raise exception 'reservation_expired'; end if;
  perform public.finish_free_usage(p_user, p_request, p_words);
  if p_words = 0 then return null; end if;
  insert into public.free_cleanup_receipts(receipt_id, user_id, week_start, transcript_hash, words)
    values(p_request, p_user, w, p_transcript_hash, p_words);
  return p_request;
end;
$$;

create function public.reserve_free_cleanup(p_user uuid, p_receipt uuid, p_request uuid,
  p_transcript_hash text, p_input integer, p_output integer) returns void
language plpgsql security definer set search_path = '' as $$
declare r public.free_cleanup_receipts; used_input bigint; used_output bigint; attempts bigint;
begin
  if p_user is null or p_receipt is null or p_request is null or p_transcript_hash is null
    or p_input is null or p_output is null or p_input < 1 or p_input > 100000
    or p_output < 1 or p_output > 8192
  then raise exception 'invalid_cleanup_reservation'; end if;
  perform pg_catalog.pg_advisory_xact_lock(pg_catalog.hashtextextended(p_user::text, 7341));
  select * into r from public.free_cleanup_receipts where receipt_id = p_receipt and user_id = p_user for update;
  if not found or r.transcript_hash <> p_transcript_hash then raise exception 'invalid_cleanup_receipt'; end if;
  if r.expires_at <= now() then raise exception 'cleanup_receipt_expired'; end if;
  if r.completed_at is not null then raise exception 'cleanup_already_completed'; end if;
  if exists(select 1 from public.free_cleanup_attempts a join public.free_cleanup_receipts c using (receipt_id)
    where c.user_id = p_user and a.state = 'reserved' and a.reserved_until > now())
  then raise exception 'request_in_progress'; end if;
  select count(*) into attempts from public.free_cleanup_attempts where receipt_id = p_receipt;
  if attempts >= 2 then raise exception 'cleanup_retry_limit'; end if;
  -- Unknown provider outcomes retain their reservation, even after the lease expires.
  select coalesce(sum(case when a.state = 'reserved' then a.reserved_input else a.input_tokens end), 0),
    coalesce(sum(case when a.state = 'reserved' then a.reserved_output else a.output_tokens end), 0)
    into used_input, used_output
    from public.free_cleanup_attempts a join public.free_cleanup_receipts c using (receipt_id)
    where c.user_id = p_user and c.week_start = r.week_start;
  -- Internal cost safeguards, independent of the public 2,000-word allowance. Ordinary
  -- cleanup is charged only here; it never increases free_weekly_usage.words or attempts.
  if used_input + p_input > 250000 or used_output + p_output > 250000
  then raise exception 'weekly_cleanup_limit'; end if;
  insert into public.free_cleanup_attempts(request_id, receipt_id, reserved_input, reserved_output)
    values(p_request, p_receipt, p_input, p_output);
end;
$$;

create function public.finish_free_cleanup(p_user uuid, p_request uuid,
  p_input integer, p_output integer, p_succeeded boolean) returns void
language plpgsql security definer set search_path = '' as $$
declare a public.free_cleanup_attempts;
begin
  if p_user is null or p_request is null or p_input is null or p_output is null or p_succeeded is null
    or p_input < 0 or p_output < 0 then raise exception 'invalid_cleanup_usage'; end if;
  perform pg_catalog.pg_advisory_xact_lock(pg_catalog.hashtextextended(p_user::text, 7341));
  select x.* into a from public.free_cleanup_attempts x join public.free_cleanup_receipts c using (receipt_id)
    where x.request_id = p_request and c.user_id = p_user for update of x;
  if not found then raise exception 'invalid_cleanup_receipt'; end if;
  if p_input > a.reserved_input or p_output > a.reserved_output then raise exception 'invalid_cleanup_usage'; end if;
  if a.state = 'finished' then
    if a.input_tokens <> p_input or a.output_tokens <> p_output or a.succeeded <> p_succeeded
    then raise exception 'reservation_completed'; end if;
    return;
  end if;
  update public.free_cleanup_attempts set state = 'finished', input_tokens = p_input,
    output_tokens = p_output, succeeded = p_succeeded, finished_at = now() where request_id = p_request;
  if p_succeeded then
    update public.free_cleanup_receipts set completed_at = coalesce(completed_at, now()) where receipt_id = a.receipt_id;
  end if;
end;
$$;

revoke all on function public.finish_free_transcription(uuid, uuid, integer, text) from public, anon, authenticated;
revoke all on function public.reserve_free_cleanup(uuid, uuid, uuid, text, integer, integer) from public, anon, authenticated;
revoke all on function public.finish_free_cleanup(uuid, uuid, integer, integer, boolean) from public, anon, authenticated;
grant execute on function public.finish_free_transcription(uuid, uuid, integer, text) to service_role;
grant execute on function public.reserve_free_cleanup(uuid, uuid, uuid, text, integer, integer) to service_role;
grant execute on function public.finish_free_cleanup(uuid, uuid, integer, integer, boolean) to service_role;
