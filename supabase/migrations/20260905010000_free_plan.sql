-- Server-owned weekly quotas. Clients cannot read or modify another user's usage.
create table public.free_weekly_usage (
  user_id uuid not null references auth.users(id) on delete cascade,
  week_start date not null,
  words integer not null default 0 check (words >= 0),
  attempts integer not null default 0 check (attempts >= 0),
  request_id uuid,
  reserved_until timestamptz,
  primary key (user_id, week_start)
);
alter table public.free_weekly_usage enable row level security;
revoke all on public.free_weekly_usage from anon, authenticated;

create function public.free_usage(p_user uuid) returns jsonb
language sql stable security definer set search_path = '' as $$
  select jsonb_build_object('usedWords', coalesce((select words from public.free_weekly_usage
    where user_id = p_user and week_start = date_trunc('week', now() at time zone 'UTC')::date), 0),
    'limitWords', 2000, 'resetsAt', (date_trunc('week', now() at time zone 'UTC') + interval '7 days') at time zone 'UTC');
$$;

-- One provider call at a time per account, including calls from different computers.
create function public.reserve_free_usage(p_user uuid, p_request uuid) returns void
language plpgsql security definer set search_path = '' as $$
declare
  w date := date_trunc('week', now() at time zone 'UTC')::date;
  u public.free_weekly_usage;
begin
  insert into public.free_weekly_usage(user_id, week_start) values(p_user, w) on conflict do nothing;
  select * into u from public.free_weekly_usage where user_id = p_user and week_start = w for update;
  if u.words >= 2000 then raise exception 'weekly_word_limit'; end if;
  if u.attempts >= 200 then raise exception 'weekly_request_limit'; end if;
  if u.reserved_until > now() then raise exception 'request_in_progress'; end if;
  update public.free_weekly_usage set request_id = p_request, reserved_until = now() + interval '3 minutes',
    attempts = attempts + 1 where user_id = p_user and week_start = w;
end;
$$;

-- The last recording is delivered in full, even if it crosses 2,000 words.
-- Subsequent recordings are refused until the next UTC week.
create function public.finish_free_usage(p_user uuid, p_request uuid, p_words integer) returns void
language plpgsql security definer set search_path = '' as $$
begin
  if p_words < 0 or p_words > 20000 then raise exception 'invalid_word_count'; end if;
  update public.free_weekly_usage set words = words + p_words, request_id = null,
    reserved_until = now() + interval '2 seconds'
    where user_id = p_user and request_id = p_request;
  if not found then raise exception 'reservation_expired'; end if;
end;
$$;

revoke all on function public.free_usage(uuid) from public, anon, authenticated;
revoke all on function public.reserve_free_usage(uuid, uuid) from public, anon, authenticated;
revoke all on function public.finish_free_usage(uuid, uuid, integer) from public, anon, authenticated;
grant execute on function public.free_usage(uuid) to service_role;
grant execute on function public.reserve_free_usage(uuid, uuid) to service_role;
grant execute on function public.finish_free_usage(uuid, uuid, integer) to service_role;
-- The original Pro helper must also be server-only.
revoke all on function public.usage_last_30_days(uuid) from public, anon, authenticated;
grant execute on function public.usage_last_30_days(uuid) to service_role;
