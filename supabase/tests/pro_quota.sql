-- Transactional assertions; requires the Pro quota migration, leaves no data.
begin;
do $$
declare
  l uuid := gen_random_uuid(); r uuid := gen_random_uuid(); r2 uuid := gen_random_uuid();
  caught boolean;
begin
  insert into public.licenses(id, key_hash, status) values(l, 'pro-test-' || l, 'active');
  perform public.reserve_pro_usage(l, r, 'transcribe', 10, 0, 0);
  if (public.pro_usage(l)->>'usedSeconds')::numeric <> 10 then raise exception 'reservation not counted'; end if;
  caught := false;
  begin perform public.reserve_pro_usage(l, r2, 'transcribe', 10, 0, 0); exception when others then
    if sqlerrm <> 'request_in_progress' then raise; end if; caught := true;
  end;
  if not caught then raise exception 'concurrency guard failed'; end if;
  perform public.finish_pro_usage(l, r, 10, 0, 0);
  caught := false;
  begin perform public.finish_pro_usage(l, r, 10, 0, 0); exception when others then
    if sqlerrm <> 'reservation_completed' then raise; end if; caught := true;
  end;
  if not caught then raise exception 'duplicate completion accepted'; end if;
  -- A normal dictation immediately follows transcription with cleanup.
  perform public.reserve_pro_usage(l, r2, 'cleanup', 0, 1000, 1024);
  perform public.finish_pro_usage(l, r2, 0, 500, 100);
  if (public.pro_usage(l)->>'inputTokens')::bigint <> 500 or (public.pro_usage(l)->>'outputTokens')::bigint <> 100 then raise exception 'cleanup settlement'; end if;
  update public.pro_usage_requests set finished_at = now() - interval '3 seconds' where license_id = l;
  r := gen_random_uuid();
  perform public.reserve_pro_usage(l, r, 'transcribe', 600, 0, 0);
  update public.pro_usage_requests set reserved_until = now() - interval '1 second' where request_id = r;
  if (public.pro_usage(l)->>'usedSeconds')::numeric <> 610 then raise exception 'abandoned request refunded'; end if;
  r2 := gen_random_uuid();
  perform public.reserve_pro_usage(l, r2, 'transcribe', 10, 0, 0);
  perform public.finish_pro_usage(l, r2, 0, 0, 0);
  if (public.pro_usage(l)->>'usedSeconds')::numeric <> 610 then raise exception 'rejected request charged'; end if;
  update public.pro_usage_requests set finished_at = now() - interval '3 seconds' where license_id = l;
  -- Legacy compressed audio can exceed reservation: actual duration is retained.
  r2 := gen_random_uuid();
  perform public.reserve_pro_usage(l, r2, 'transcribe', 600, 0, 0);
  perform public.finish_pro_usage(l, r2, 216001, 0, 0);
  update public.pro_usage_requests set finished_at = now() - interval '3 seconds' where license_id = l;
  caught := false;
  begin perform public.reserve_pro_usage(l, gen_random_uuid(), 'transcribe', 10, 0, 0); exception when others then
    if sqlerrm <> 'monthly_audio_limit' then raise; end if; caught := true;
  end;
  if not caught then raise exception 'audio quota failed'; end if;
  update public.pro_usage_requests set created_at = now() - interval '31 days' where license_id = l;
  if (public.pro_usage(l)->>'usedSeconds')::numeric <> 0 then raise exception 'rolling expiry failed'; end if;
  r := gen_random_uuid();
  perform public.reserve_pro_usage(l, r, 'cleanup', 0, 1000, 1024);
  perform public.finish_pro_usage(l, r, 0, 1000, 1024);
  update public.pro_usage_requests set input_tokens = 3000000, finished_at = now() - interval '3 seconds' where request_id = r;
  caught := false;
  begin perform public.reserve_pro_usage(l, gen_random_uuid(), 'cleanup', 0, 1, 1); exception when others then
    if sqlerrm <> 'monthly_cleanup_limit' then raise; end if; caught := true;
  end;
  if not caught then raise exception 'cleanup quota failed'; end if;
  -- Cleanup exhaustion must not prevent transcription within its own allowance.
  r2 := gen_random_uuid();
  perform public.reserve_pro_usage(l, r2, 'transcribe', 10, 0, 0);
  perform public.finish_pro_usage(l, r2, 10, 0, 0);
  update public.pro_usage_requests set created_at = now() - interval '31 days', finished_at = now() - interval '3 seconds' where license_id = l;
  insert into public.usage_events(license_id, seconds, kind) values(l, 216000, 'transcribe');
  caught := false;
  begin perform public.reserve_pro_usage(l, gen_random_uuid(), 'transcribe', 10, 0, 0); exception when others then
    if sqlerrm <> 'monthly_audio_limit' then raise; end if; caught := true;
  end;
  if not caught then raise exception 'legacy audio omitted'; end if;
  delete from public.usage_events where license_id = l;
  insert into public.pro_usage_requests(request_id, license_id, kind, state, finished_at)
    select gen_random_uuid(), l, 'transcribe', 'finished', now() - interval '3 seconds' from generate_series(1, 12000);
  caught := false;
  begin perform public.reserve_pro_usage(l, gen_random_uuid(), 'transcribe', 10, 0, 0); exception when others then
    if sqlerrm <> 'monthly_request_limit' then raise; end if; caught := true;
  end;
  if not caught then raise exception 'request quota failed'; end if;
  if has_function_privilege('anon', 'public.pro_usage(uuid)', 'execute')
    or has_function_privilege('authenticated', 'public.reserve_pro_usage(uuid,uuid,text,numeric,bigint,bigint)', 'execute')
    or has_table_privilege('authenticated', 'public.pro_usage_requests', 'insert') then raise exception 'client quota permission'; end if;
  raise notice 'PASS: Pro reservations, concurrency, settlement, duplicates, uncertainty, limits, legacy usage and service-only access';
end $$;
rollback;
