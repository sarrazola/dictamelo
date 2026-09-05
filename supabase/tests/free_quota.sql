-- Runs entirely in a transaction and leaves no test users or usage behind.
begin;
do $$
declare
  u uuid := gen_random_uuid(); r uuid := gen_random_uuid(); r2 uuid := gen_random_uuid();
  w date := date_trunc('week', now() at time zone 'UTC')::date;
  caught boolean;
begin
  insert into auth.users(id, email) values (u, 'quota-test-' || u || '@example.invalid');
  if (public.free_usage(u)->>'usedWords')::int <> 0 then raise exception 'new user quota'; end if;
  perform public.reserve_free_usage(u, r);
  caught := false;
  begin perform public.reserve_free_usage(u, r2); exception when others then
    if sqlerrm <> 'request_in_progress' then raise; end if; caught := true;
  end;
  if not caught then raise exception 'concurrency guard failed'; end if;
  perform public.finish_free_usage(u, r, 2001);
  if (public.free_usage(u)->>'usedWords')::int <> 2001 then raise exception 'word accounting'; end if;
  caught := false;
  begin perform public.reserve_free_usage(u, r2); exception when others then
    if sqlerrm <> 'weekly_word_limit' then raise; end if; caught := true;
  end;
  if not caught then raise exception 'weekly limit failed'; end if;
  caught := false;
  begin perform public.finish_free_usage(u, r, 2001); exception when others then
    if sqlerrm <> 'reservation_expired' then raise; end if; caught := true;
  end;
  if not caught then raise exception 'duplicate completion charged twice'; end if;
  update public.free_weekly_usage set week_start = w - 7 where user_id = u;
  if (public.free_usage(u)->>'usedWords')::int <> 0 then raise exception 'weekly reset failed'; end if;
  perform public.reserve_free_usage(u, r2);
  perform public.finish_free_usage(u, r2, 0);
  if (public.free_usage(u)->>'usedWords')::int <> 0 then raise exception 'failed request charged'; end if;
  update public.free_weekly_usage set attempts = 200, reserved_until = now() - interval '1 minute'
    where user_id = u and week_start = w;
  caught := false;
  begin perform public.reserve_free_usage(u, gen_random_uuid()); exception when others then
    if sqlerrm <> 'weekly_request_limit' then raise; end if; caught := true;
  end;
  if not caught then raise exception 'request cap failed'; end if;
  if has_function_privilege('anon', 'public.free_usage(uuid)', 'execute') then raise exception 'anon can access quota'; end if;
  if has_function_privilege('authenticated', 'public.reserve_free_usage(uuid,uuid)', 'execute') then raise exception 'client can edit quota'; end if;
  raise notice 'PASS: accounting, concurrency, limit, duplicate completion, renewal, failure, rate cap and permissions';
end $$;
rollback;
