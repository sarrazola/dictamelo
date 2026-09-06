-- Every assertion rolls back. Audio seconds and legacy word telemetry are independent.
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
do $$
declare
  u uuid := gen_random_uuid(); r uuid := gen_random_uuid(); r2 uuid := gen_random_uuid();
  old_user uuid := gen_random_uuid(); old_request uuid := gen_random_uuid();
  w date := date_trunc('week', now() at time zone 'UTC')::date;
begin
  insert into auth.users(id,email) values(u,'quota-test-'||u||'@example.invalid');
  if (public.free_usage(u)->>'usedSeconds')::numeric <> 0 or (public.free_usage(u)->>'limitSeconds')::numeric <> 1800
    or (public.free_usage(u)->>'limitWords')::int <> 2000 then raise exception 'new account allowance or compatibility'; end if;
  perform public.reserve_free_audio(u,r,5.855);
  if (public.free_usage(u)->>'usedSeconds')::numeric <> 5.855 then raise exception 'reservation not counted'; end if;
  perform pg_temp.expect_error(format('select public.reserve_free_audio(%L,%L,10)',u,r2),'request_in_progress');
  perform public.finish_free_transcription(u,r,17,repeat('a',64));
  perform public.finish_free_transcription(u,r,17,repeat('a',64));
  if (public.free_usage(u)->>'usedSeconds')::numeric <> 5.855 or (public.free_usage(u)->>'usedWords')::int <> 17
    then raise exception 'exact PCM duration or duplicate settlement'; end if;
  perform pg_temp.expect_error(format('select public.finish_free_transcription(%L,%L,18,%L)',u,r,repeat('a',64)),'invalid_cleanup_receipt');
  -- The last accepted recording can cross the allowance; its receipt still works.
  update public.free_weekly_usage set legacy_audio_seconds=1799-5.855,reserved_until=now()-interval '1 second' where user_id=u;
  perform public.reserve_free_audio(u,r2,5.855);
  perform public.finish_free_transcription(u,r2,17,repeat('a',64));
  if (public.free_usage(u)->>'usedSeconds')::numeric <> 1804.855 then raise exception 'last recording truncated'; end if;
  perform pg_temp.expect_error(format('select public.reserve_free_audio(%L,%L,1)',u,gen_random_uuid()),'weekly_audio_limit');
  perform public.reserve_free_cleanup(u,r2,gen_random_uuid(),repeat('a',64),1000,1024);
  if (public.free_usage(u)->>'usedSeconds')::numeric <> 1804.855 then raise exception 'cleanup charged audio'; end if;
  -- Unknown outcomes retain their seconds after the lease expires. A late finish
  -- still settles its own ledger entry even after the next request was reserved.
  update public.free_weekly_usage set legacy_audio_seconds=0,reserved_until=now()-interval '1 second' where user_id=u;
  r:=gen_random_uuid(); r2:=gen_random_uuid();
  perform public.reserve_free_audio(u,r,20);
  update public.free_audio_requests set reserved_until=now()-interval '1 second' where request_id=r;
  update public.free_weekly_usage set reserved_until=now()-interval '1 second' where user_id=u;
  perform public.reserve_free_audio(u,r2,30);
  perform public.finish_free_transcription(u,r,2,repeat('a',64));
  perform pg_temp.expect_error(format('select public.reserve_free_audio(%L,%L,1)',u,gen_random_uuid()),'request_in_progress');
  perform public.finish_free_usage(u,r2,0);
  if (public.free_usage(u)->>'usedSeconds')::numeric <> 31.710 then raise exception 'uncertainty/late completion/refund accounting'; end if;
  if (select attempts from public.free_weekly_usage where user_id=u and week_start=w) <> 4 then raise exception 'refund reset attempts'; end if;
  -- Silence is successful provider work even when no cleanup receipt is useful.
  update public.free_weekly_usage set reserved_until=now()-interval '1 second' where user_id=u;
  r:=gen_random_uuid(); perform public.reserve_free_audio(u,r,5);
  if public.finish_free_transcription(u,r,0,repeat('a',64)) is not null then raise exception 'silence receipt'; end if;
  if (public.free_usage(u)->>'usedSeconds')::numeric <> 36.710 then raise exception 'silence escaped audio quota'; end if;
  -- Old signatures remain usable and preserve word telemetry without enforcing it.
  update public.free_weekly_usage set reserved_until=now()-interval '1 second' where user_id=u;
  r:=gen_random_uuid(); perform public.reserve_free_usage(u,r); perform public.finish_free_usage(u,r,2001);
  if (public.free_usage(u)->>'usedWords')::int <> 2037 or (public.free_usage(u)->>'usedSeconds')::numeric <> 156.710
    then raise exception 'legacy caller compatibility'; end if;
  update public.free_weekly_usage set attempts=1000,reserved_until=now()-interval '1 second' where user_id=u;
  perform pg_temp.expect_error(format('select public.reserve_free_audio(%L,%L,1)',u,gen_random_uuid()),'weekly_request_limit');
  perform pg_temp.expect_error(format('select public.reserve_free_audio(%L,%L,121)',u,gen_random_uuid()),'invalid_free_reservation');
  perform pg_temp.expect_error(format('select public.reserve_free_audio(%L,%L,''NaN'')',u,gen_random_uuid()),'invalid_free_reservation');
  -- A request crossing Monday belongs to its original week and still blocks a
  -- second machine until it completes. Its late completion does not spend new time.
  insert into auth.users(id,email) values(old_user,'week-test-'||old_user||'@example.invalid');
  insert into public.free_weekly_usage(user_id,week_start,request_id,reserved_until) values(old_user,w-7,old_request,now()+interval '1 minute');
  insert into public.free_audio_requests(request_id,user_id,week_start,reserved_seconds) values(old_request,old_user,w-7,5.855);
  perform pg_temp.expect_error(format('select public.reserve_free_audio(%L,%L,1)',old_user,gen_random_uuid()),'request_in_progress');
  perform public.finish_free_transcription(old_user,old_request,17,repeat('a',64));
  if (public.free_usage(old_user)->>'usedSeconds')::numeric <> 0 then raise exception 'late completion charged new week'; end if;
  update public.free_weekly_usage set reserved_until=now()-interval '1 second' where user_id=old_user;
  perform public.reserve_free_audio(old_user,gen_random_uuid(),5.855);
  if (public.free_usage(old_user)->>'usedSeconds')::numeric <> 5.855 then raise exception 'new week audio allowance'; end if;
  if has_function_privilege('anon','public.free_usage(uuid)','execute')
    or has_function_privilege('authenticated','public.reserve_free_audio(uuid,uuid,numeric)','execute')
    or has_function_privilege('authenticated','public.settle_free_audio(uuid,uuid,integer,boolean)','execute')
    or has_table_privilege('authenticated','public.free_audio_requests','insert') then raise exception 'client quota permission'; end if;
  raise notice 'PASS: measured seconds, reservations, last recording, cleanup, failures, late completion, silence, legacy callers, attempts and permissions';
end $$;
rollback;
