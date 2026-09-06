#!/usr/bin/env python3
"""Opt-in Free Cloud regression using the licensed speech fixture and disposable accounts.

Run: python3 scripts/test-free-cleanup-live.py --live --project-ref YOUR_PROJECT_REF
Requires requests and an authenticated Supabase CLI. Sends only the committed public audio,
creates two synthetic accounts without SMTP, and deletes only those accounts afterward.
Credentials, sessions and OTPs remain in memory; the app Keychain is never accessed.
"""
from concurrent.futures import ThreadPoolExecutor
import argparse
import hashlib
import math
from pathlib import Path
import re
import runpy
import secrets
import sys
import uuid

import requests

from audio_fixture import assert_transcript, load_fixture

ROOT = Path(__file__).resolve().parents[1]
AUTH = runpy.run_path(str(ROOT / "scripts/test-auth-live.py"))
TestFailure = AUTH["TestFailure"]
require = AUTH["require"]


def request(session, base, method, path, stage, *, expected=(200,), **kwargs):
    try:
        response = session.request(method, base + path, timeout=120, **kwargs)
    except requests.RequestException:
        raise TestFailure(f"{stage}: network request failed") from None
    require(response.status_code in expected, f"{stage}: unexpected HTTP {response.status_code}")
    if not response.content:
        return response.status_code, None
    try:
        result = response.json()
    except ValueError:
        raise TestFailure(f"{stage}: invalid JSON response") from None
    return response.status_code, result


def run(project):
    fixture = load_fixture()
    base = f"https://{project}.supabase.co"
    anon, service = AUTH["get_keys"](project)
    admin, public = requests.Session(), requests.Session()
    admin.headers.update(apikey=service, Authorization="Bearer " + service)
    public.headers.update(apikey=anon)
    created = []
    try:
        # Fail before making test identities or provider calls if the migration is absent.
        request(admin, base, "GET", "/rest/v1/free_cleanup_receipts?select=receipt_id&limit=0", "Check cleanup migration")
        request(admin, base, "GET", "/rest/v1/free_audio_requests?select=request_id&limit=0", "Check audio-time migration")
        _, settings = request(public, base, "GET", "/auth/v1/settings", "Check email confirmations")
        require(settings.get("mailer_autoconfirm") is False, "Email confirmation must remain enabled")
        for _ in range(2):
            email = f"dictamelo-cleanup-test-{uuid.uuid4()}@example.invalid"
            _, generated = request(admin, base, "POST", "/auth/v1/admin/generate_link", "Create synthetic account",
                                   json={"type": "signup", "email": email, "password": "Aa1!" + secrets.token_urlsafe(32)})
            identifier = AUTH["user_id"](generated)
            account = {"id": identifier, "email": email}
            created.append(account)
            require(generated.get("user", generated).get("email") == email, "Synthetic account email mismatch")
            _, verified = request(public, base, "POST", "/auth/v1/verify", "Verify synthetic account without email",
                                  json={"type": "signup", "email": email, "token": AUTH["otp"](generated)})
            account["token"] = AUTH["assert_session"](verified, identifier)["access_token"]
        first, other = created

        def user_headers(account):
            return {"Authorization": "Bearer " + account["token"]}

        def usage(account):
            return request(public, base, "POST", "/functions/v1/usage", "Read weekly audio allowance",
                           headers=user_headers(account), json={})[1]

        def attempts(receipt):
            return request(admin, base, "GET", "/rest/v1/free_cleanup_attempts?receipt_id=eq." + receipt +
                           "&select=request_id,state,input_tokens,output_tokens,succeeded", "Read synthetic attempt accounting")[1]

        def transcribe(account, expected=(200,)):
            with fixture.path.open("rb") as audio:
                return request(public, base, "POST", "/functions/v1/transcribe", "Upload licensed fixture",
                               expected=expected, headers=user_headers(account), data={"language": "en"},
                               files={"file": (fixture.path.name, audio, "audio/wav")})

        require(usage(first)["usedSeconds"] == 0 and usage(first)["limitSeconds"] == 1800, "Initial 30-minute allowance mismatch")
        require(usage(first)["usedWords"] == 0 and usage(first)["limitWords"] == 2000, "Legacy usage fields disappeared")
        request(public, base, "POST", "/functions/v1/cleanup", "Reject unauthenticated cleanup", expected=(401,), json={})
        request(public, base, "POST", "/functions/v1/cleanup", "Reject invalid JWT", expected=(401,),
                headers={"Authorization": "Bearer deliberately-invalid"}, json={})
        request(public, base, "POST", "/functions/v1/cleanup", "Reject absent receipt", expected=(400,),
                headers=user_headers(first), json={"text": "not a transcription"})
        _, transcription = transcribe(first)
        text = transcription.get("text")
        metrics = assert_transcript(text, fixture.transcript)
        receipt = transcription.get("cleanupReceipt")
        require(isinstance(receipt, str) and str(uuid.UUID(receipt)) == receipt, "Transcription omitted cleanup receipt")
        _, receipt_rows = request(admin, base, "GET", "/rest/v1/free_cleanup_receipts?receipt_id=eq." + receipt +
                                  "&select=user_id,words,transcript_hash", "Read synthetic receipt")
        require(len(receipt_rows) == 1 and receipt_rows[0]["user_id"] == first["id"], "Receipt owner mismatch")
        require(receipt_rows[0]["transcript_hash"] == hashlib.sha256(text.encode()).hexdigest(), "Receipt does not bind canonical text")
        charged = receipt_rows[0]["words"]
        require(charged > 0 and usage(first)["usedWords"] == charged, "Transcription was not charged exactly once")
        require(math.isclose(usage(first)["usedSeconds"], fixture.duration_seconds, abs_tol=0.0001), "PCM duration was not charged exactly once")
        require(math.isclose(transcription["duration"], fixture.duration_seconds, abs_tol=0.0001), "Response duration differs from validated PCM")
        payload = {"text": text, "cleanupReceipt": receipt}
        request(public, base, "POST", "/functions/v1/cleanup", "Reject changed transcript", expected=(403,),
                headers=user_headers(first), json={**payload, "text": text + " Changed transcript."})
        request(public, base, "POST", "/functions/v1/cleanup", "Reject another account receipt", expected=(403,),
                headers=user_headers(other), json=payload)
        require(attempts(receipt) == [], "Rejected cleanup created provider attempts")

        rpc_body = {"p_user": first["id"], "p_receipt": receipt, "p_request": str(uuid.uuid4()),
                    "p_transcript_hash": receipt_rows[0]["transcript_hash"], "p_input": 1000, "p_output": 1024}
        for headers in ({}, user_headers(first)):
            request(public, base, "POST", "/rest/v1/rpc/reserve_free_cleanup", "Reject direct client quota access",
                    expected=(401, 403), headers=headers, json=rpc_body)
        print(f"PASS raw fixture: {metrics['actual_words']} words, WER={metrics['word_error_rate']:.3f}; owner/hash/auth/RPC rejections create zero attempts")

        def cleanup_claim(_):
            with requests.Session() as client:
                client.headers.update(apikey=anon, **user_headers(first))
                return request(client, base, "POST", "/functions/v1/cleanup", "Concurrent cleanup claim",
                               expected=(200, 409), json=payload)

        with ThreadPoolExecutor(max_workers=2) as pool:
            replies = list(pool.map(cleanup_claim, range(2)))
        require(sorted(status for status, _ in replies) == [200, 409], "Concurrent cleanup did not yield exactly one success")
        cleaned = next(body for status, body in replies if status == 200)
        require(cleaned.get("model") == "openai/gpt-oss-20b", "Cleanup returned an unexpected provider model")
        assert_transcript(cleaned["choices"][0]["message"]["content"], fixture.transcript)
        rows = attempts(receipt)
        require(len(rows) == 1 and rows[0]["state"] == "finished" and rows[0]["succeeded"] is True,
                "Concurrent calls consumed more than one provider attempt")
        require(rows[0]["input_tokens"] > 0 and rows[0]["output_tokens"] > 0, "Actual cleanup tokens were not settled")
        request(public, base, "POST", "/functions/v1/cleanup", "Reject successful replay", expected=(409,),
                headers=user_headers(first), json=payload)
        require(len(attempts(receipt)) == 1 and usage(first)["usedWords"] == charged, "Replay consumed tokens or words")
        require(usage(other)["usedWords"] == 0, "Cross-account request changed another account's words")
        require(math.isclose(usage(first)["usedSeconds"], fixture.duration_seconds, abs_tol=0.0001), "Cleanup changed audio allowance")
        print(f"PASS live cleanup model={cleaned['model']}: race 200/409, replay 409, one provider attempt; {charged} words and {fixture.duration_seconds}s counted once")

        # Move only this new synthetic account to the public boundary. Real account rows are never edited.
        _, weeks = request(admin, base, "GET", "/rest/v1/free_weekly_usage?user_id=eq." + first["id"] +
                           "&select=week_start", "Read synthetic quota week")
        require(len(weeks) == 1, "Synthetic account has unexpected quota weeks")
        request(admin, base, "PATCH", "/rest/v1/free_weekly_usage?user_id=eq." + first["id"] +
                "&week_start=eq." + weeks[0]["week_start"], "Prepare synthetic 30-minute boundary", expected=(204,),
                json={"legacy_audio_seconds": 1799 - fixture.duration_seconds, "reserved_until": "2000-01-01T00:00:00Z"})
        _, final = transcribe(first)
        assert_transcript(final["text"], fixture.transcript)
        before_cleanup = usage(first)["usedSeconds"]
        require(math.isclose(before_cleanup, 1799 + fixture.duration_seconds, abs_tol=0.0001), "Final complete recording did not cross the expected boundary")
        _, final_cleaned = request(public, base, "POST", "/functions/v1/cleanup", "Clean final over-limit recording",
                                   headers=user_headers(first), json={"text": final["text"], "cleanupReceipt": final["cleanupReceipt"]})
        require(final_cleaned.get("model") == "openai/gpt-oss-20b", "Final cleanup model mismatch")
        assert_transcript(final_cleaned["choices"][0]["message"]["content"], fixture.transcript)
        require(usage(first)["usedSeconds"] == before_cleanup, "Final cleanup double-counted audio")
        transcribe(first, expected=(429,))
        require(usage(first)["usedSeconds"] == before_cleanup, "Rejected exhausted recording changed audio")
        print(f"PASS weekly boundary: final recording delivered and cleaned at {before_cleanup}/1800 seconds; next transcription 429; cleanup adds zero audio time")
    finally:
        cleanup_failed = False
        for account in reversed(created):
            try:
                _, current = request(admin, base, "GET", "/auth/v1/admin/users/" + account["id"], "Verify synthetic identity before cleanup")
                require(AUTH["user_id"](current) == account["id"] and current.get("user", current).get("email") == account["email"],
                        "Refused cleanup of mismatching identity")
                request(admin, base, "DELETE", "/auth/v1/admin/users/" + account["id"], "Delete synthetic account", expected=(200, 204))
                request(admin, base, "GET", "/auth/v1/admin/users/" + account["id"], "Verify synthetic deletion", expected=(404,))
                for table in ("free_weekly_usage", "free_cleanup_receipts", "free_audio_requests"):
                    _, rows = request(admin, base, "GET", "/rest/v1/" + table + "?user_id=eq." + account["id"], "Verify cascade cleanup")
                    require(rows == [], "Synthetic quota records remained after deletion")
            except Exception:
                print(f"CLEANUP REQUIRED: synthetic account {account['id']}", file=sys.stderr)
                cleanup_failed = True
        admin.close()
        public.close()
        if cleanup_failed:
            raise TestFailure("Synthetic cleanup was not fully verified")
        if created:
            print(f"CLEANUP: deleted exactly {len(created)} newly created synthetic accounts; verified account/quota/receipt removal")
    print("LIMIT: hosted API and committed file only; SMTP, physical microphone, paste and native UI are not covered")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--live", action="store_true")
    parser.add_argument("--project-ref", required=True)
    args = parser.parse_args()
    if not args.live or not re.fullmatch(r"[a-z]{20}", args.project_ref):
        parser.error("Pass --live and an explicit 20-letter --project-ref.")
    try:
        run(args.project_ref)
    except TestFailure as error:
        print(f"FAIL: {error}", file=sys.stderr)
        return 1
    except Exception:
        print("FAIL: unexpected regression error; response bodies and credentials suppressed", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
