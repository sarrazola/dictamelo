-- Service-side regression assertions; the transaction leaves no users, words or receipts behind.
begin;
create function pg_temp.expect_error(statement text, expected text) returns void language plpgsql as $$
begin
  begin execute statement;
  exception when others then
    if sqlerrm <> expected then raise exception 'Expected %, received %', expected, sqlerrm; end if;
    return;
  end;
  raise exception 'Expected rejection: %', expected;
end;
$$;
create function pg_temp.make_receipt(u uuid, r uuid, words integer) returns uuid language plpgsql as $$
begin
  update public.free_weekly_usage set reserved_until = now() - interval '1 minute' where user_id = u;
  perform public.reserve_free_usage(u, r);
  return public.finish_free_transcription(u, r, words, repeat('a', 64));
end;
$$;
do $$
declare
  u uuid := gen_random_uuid(); other_user uuid := gen_random_uuid(); budget_user uuid := gen_random_uuid();
  r uuid := gen_random_uuid(); r2 uuid := gen_random_uuid(); r3 uuid := gen_random_uuid();
  a uuid := gen_random_uuid(); a2 uuid := gen_random_uuid(); a3 uuid := gen_random_uuid();
  before_words integer; receipt uuid;
begin
  insert into auth.users(id, email) values
    (u, 'free-cleanup-test-' || u || '@example.invalid'),
    (other_user, 'free-cleanup-test-' || other_user || '@example.invalid'),
    (budget_user, 'free-cleanup-test-' || budget_user || '@example.invalid');
  receipt := pg_temp.make_receipt(u, r, 2001);
  if receipt <> r or (public.free_usage(u)->>'usedWords')::int <> 2001 then raise exception 'receipt word accounting'; end if;
  if public.finish_free_transcription(u, r, 2001, repeat('a', 64)) <> r then raise exception 'completion retry not idempotent'; end if;
  if (public.free_usage(u)->>'usedWords')::int <> 2001 then raise exception 'transcription charged twice'; end if;
  perform pg_temp.expect_error(format('select public.finish_free_transcription(%L,%L,2001,%L)',u,r,repeat('b',64)), 'invalid_cleanup_receipt');
  perform pg_temp.expect_error(format('select public.reserve_free_usage(%L,%L)',u,gen_random_uuid()), 'weekly_word_limit');
  perform pg_temp.expect_error(format('select public.reserve_free_cleanup(%L,%L,%L,%L,1000,1024)',other_user,r,a,repeat('a',64)), 'invalid_cleanup_receipt');
  perform pg_temp.expect_error(format('select public.reserve_free_cleanup(%L,%L,%L,%L,1000,1024)',u,r,a,repeat('b',64)), 'invalid_cleanup_receipt');
  -- Cleanup of the final over-limit recording remains available immediately and costs no words.
  perform public.reserve_free_cleanup(u,r,a,repeat('a',64),1000,1024);
  perform pg_temp.expect_error(format('select public.reserve_free_cleanup(%L,%L,%L,%L,1000,1024)',u,r,a2,repeat('a',64)), 'request_in_progress');
  perform pg_temp.expect_error(format('select public.finish_free_cleanup(%L,%L,1,1,true)',other_user,a), 'invalid_cleanup_receipt');
  perform pg_temp.expect_error(format('select public.finish_free_cleanup(%L,%L,1001,1,true)',u,a), 'invalid_cleanup_usage');
  perform public.finish_free_cleanup(u,a,100,20,true);
  perform public.finish_free_cleanup(u,a,100,20,true);
  perform pg_temp.expect_error(format('select public.finish_free_cleanup(%L,%L,101,20,true)',u,a), 'reservation_completed');
  perform pg_temp.expect_error(format('select public.reserve_free_cleanup(%L,%L,%L,%L,1000,1024)',u,r,a2,repeat('a',64)), 'cleanup_already_completed');
  if (public.free_usage(u)->>'usedWords')::int <> 2001 then raise exception 'cleanup changed words'; end if;
  if (select count(*) from public.free_cleanup_attempts where receipt_id=r) <> 1 then raise exception 'replay created provider attempt'; end if;

  -- Failed/empty transcription settles zero words but cannot mint cleanup access.
  if pg_temp.make_receipt(other_user,r2,0) is not null then raise exception 'empty transcription minted receipt'; end if;
  if exists(select 1 from public.free_cleanup_receipts where receipt_id=r2) then raise exception 'empty receipt exists'; end if;
  r2 := gen_random_uuid(); r3 := gen_random_uuid(); a := gen_random_uuid(); a2 := gen_random_uuid(); a3 := gen_random_uuid();
  perform pg_temp.make_receipt(other_user,r2,10);
  perform pg_temp.make_receipt(other_user,r3,10);
  before_words := (public.free_usage(other_user)->>'usedWords')::int;
  perform public.reserve_free_cleanup(other_user,r2,a,repeat('a',64),1000,1024);
  perform pg_temp.expect_error(format('select public.reserve_free_cleanup(%L,%L,%L,%L,1000,1024)',other_user,r3,a2,repeat('a',64)), 'request_in_progress');
  update public.free_cleanup_attempts set reserved_until=now()-interval '1 second' where request_id=a;
  perform public.reserve_free_cleanup(other_user,r2,a2,repeat('a',64),1000,1024);
  -- A timed-out first attempt still consumes its conservative cost reservation.
  if (select sum(reserved_input) from public.free_cleanup_attempts where receipt_id=r2 and state='reserved') <> 2000
  then raise exception 'uncertain reservation was released'; end if;
  perform public.finish_free_cleanup(other_user,a2,0,0,false);
  perform pg_temp.expect_error(format('select public.reserve_free_cleanup(%L,%L,%L,%L,1000,1024)',other_user,r2,a3,repeat('a',64)), 'cleanup_retry_limit');
  if (public.free_usage(other_user)->>'usedWords')::int <> before_words then raise exception 'cleanup retry changed words'; end if;
  update public.free_cleanup_receipts set expires_at=now()-interval '1 second' where receipt_id=r3;
  perform pg_temp.expect_error(format('select public.reserve_free_cleanup(%L,%L,%L,%L,1000,1024)',other_user,r3,a3,repeat('a',64)), 'cleanup_receipt_expired');

  -- Expired leases remain included in the server-owned weekly money cap.
  for i in 1..3 loop
    r := gen_random_uuid(); a := gen_random_uuid();
    perform pg_temp.make_receipt(budget_user,r,1);
    if i < 3 then
      perform public.reserve_free_cleanup(budget_user,r,a,repeat('a',64),100000,8192);
      update public.free_cleanup_attempts set reserved_until=now()-interval '1 second' where request_id=a;
    else
      perform pg_temp.expect_error(format('select public.reserve_free_cleanup(%L,%L,%L,%L,100000,8192)',budget_user,r,a,repeat('a',64)), 'weekly_cleanup_limit');
      perform pg_temp.expect_error(format('select public.reserve_free_cleanup(%L,%L,%L,%L,1,8193)',budget_user,r,a,repeat('a',64)), 'invalid_cleanup_reservation');
    end if;
  end loop;
  if (select count(*) from public.free_cleanup_attempts a join public.free_cleanup_receipts r using(receipt_id) where r.user_id=budget_user) <> 2
  then raise exception 'rejected request consumed attempt'; end if;

  if has_table_privilege('authenticated','public.free_cleanup_receipts','select')
    or has_table_privilege('anon','public.free_cleanup_attempts','insert')
    or has_function_privilege('anon','public.finish_free_transcription(uuid,uuid,integer,text)','execute')
    or has_function_privilege('authenticated','public.reserve_free_cleanup(uuid,uuid,uuid,text,integer,integer)','execute')
    or has_function_privilege('authenticated','public.finish_free_cleanup(uuid,uuid,integer,integer,boolean)','execute')
  then raise exception 'client can access cleanup accounting'; end if;
  if not has_function_privilege('service_role','public.reserve_free_cleanup(uuid,uuid,uuid,text,integer,integer)','execute')
  then raise exception 'service role cannot reserve cleanup'; end if;
  raise notice 'PASS: atomic receipt/words, owner/hash binding, replay, concurrent claims, uncertain costs, bounded retry, expiry, weekly cap and permissions';
end;
$$;
rollback;
