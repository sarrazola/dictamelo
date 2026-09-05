#!/usr/bin/env python3
"""Opt-in live Supabase auth contract test using one disposable synthetic account.

Usage: python3 scripts/test-auth-live.py --live --project-ref YOUR_PROJECT_REF
Requires Python requests and an authenticated Supabase CLI. CLI keys, passwords, OTPs,
and sessions stay in process memory; no real email is sent and app Keychain is untouched.
Admin generate_link supplies signup/recovery codes. This does NOT test SMTP delivery,
the public signup email-send operation, Google consent, or the native UI/credential store.
"""
from __future__ import annotations

import argparse
import json
import re
import secrets
import subprocess
import sys
import uuid

import requests


class TestFailure(RuntimeError):
    """Failure text contains only fixed stage names and HTTP status codes."""


def require(condition: bool, stage: str) -> None:
    if not condition:
        raise TestFailure(stage)


def api(session: requests.Session, base: str, method: str, path: str, stage: str, *, expected=(200,), **kwargs):
    try:
        response = session.request(method, base + path, timeout=30, **kwargs)
    except requests.RequestException:
        raise TestFailure(f"{stage}: network request failed") from None
    require(response.status_code in expected, f"{stage}: unexpected HTTP {response.status_code}")
    if response.status_code == 204 or not response.content:
        return {}
    try:
        data = response.json()
    except (ValueError, requests.RequestException):
        raise TestFailure(f"{stage}: response was not valid JSON") from None
    require(isinstance(data, dict), f"{stage}: response was not an object")
    return data


def get_keys(project: str):
    try:
        result = subprocess.run(["supabase", "projects", "api-keys", "--project-ref", project, "-o", "json"], capture_output=True, text=True, timeout=60)
    except (OSError, subprocess.TimeoutExpired):
        raise TestFailure("Supabase CLI could not retrieve project keys") from None
    require(result.returncode == 0, "Supabase CLI could not retrieve project keys; authenticate first")
    try:
        values = json.loads(result.stdout)
        anon = next(item["api_key"] for item in values if item["name"] == "anon")
        service = next(item["api_key"] for item in values if item["name"] == "service_role")
    except (ValueError, KeyError, TypeError, StopIteration):
        raise TestFailure("Supabase CLI did not return the required project keys") from None
    return anon, service


def user_id(data):
    user = data.get("user", data)
    require(isinstance(user, dict) and isinstance(user.get("id"), str), "Account response omitted its user ID")
    try:
        return str(uuid.UUID(user["id"]))
    except ValueError:
        raise TestFailure("Account response returned an invalid user ID") from None


def otp(data):
    value = data.get("email_otp") or data.get("properties", {}).get("email_otp")
    require(isinstance(value, str) and bool(re.fullmatch(r"[0-9]{6,10}", value)), "Code generation omitted its email verification code")
    return value


def assert_session(data, expected_id):
    require(user_id(data) == expected_id, "Auth operation returned an unexpected account")
    require(bool(data.get("access_token")) and bool(data.get("refresh_token")), "Auth operation omitted session credentials")
    return data


def run(project: str) -> None:
    base = f"https://{project}.supabase.co"
    anon, service = get_keys(project)
    admin = requests.Session()
    admin.headers.update(apikey=service, Authorization="Bearer " + service)
    public = requests.Session()
    public.headers.update(apikey=anon)
    email = f"dictamelo-auth-test-{uuid.uuid4()}@example.invalid"
    password = "Aa1!" + secrets.token_urlsafe(32)
    new_password = "Bb2!" + secrets.token_urlsafe(32)
    created_id = None
    try:
        settings = api(public, base, "GET", "/auth/v1/settings", "Read auth settings")
        require(settings.get("mailer_autoconfirm") is False, "Email confirmation must remain enabled during this test")
        generated = api(admin, base, "POST", "/auth/v1/admin/generate_link", "Create disposable signup", json={"type": "signup", "email": email, "password": password})
        created_id = user_id(generated)
        require(generated.get("user", generated).get("email") == email, "Created account email did not match the disposable address")
        confirmation_code = otp(generated)

        before = api(public, base, "POST", "/auth/v1/token?grant_type=password", "Reject unconfirmed login", expected=(400, 401, 403), json={"email": email, "password": password})
        require(before.get("error_code") == "email_not_confirmed", "Unconfirmed account did not require email confirmation")
        confirmed = assert_session(api(public, base, "POST", "/auth/v1/verify", "Confirm signup", json={"type": "signup", "email": email, "token": confirmation_code}), created_id)
        require(bool(confirmed["user"].get("email_confirmed_at")), "Confirmation did not mark the email as verified")
        replay = api(public, base, "POST", "/auth/v1/verify", "Reject confirmation replay", expected=(400, 401, 403, 422), json={"type": "signup", "email": email, "token": confirmation_code})
        require(not replay.get("access_token"), "A used confirmation code returned a session")
        print("PASS: confirmation required, signup code verified once, replay rejected")

        signed_in = assert_session(api(public, base, "POST", "/auth/v1/token?grant_type=password", "Password login", json={"email": email, "password": password}), created_id)
        wrong = api(public, base, "POST", "/auth/v1/token?grant_type=password", "Reject incorrect password", expected=(400, 401, 403), json={"email": email, "password": "wrong-password-never-valid"})
        require(wrong.get("error_code") == "invalid_credentials", "Incorrect password was not rejected as invalid credentials")
        usage = api(public, base, "POST", "/functions/v1/usage", "Read free usage", headers={"Authorization": "Bearer " + signed_in["access_token"]}, json={})
        require(usage.get("limitWords") == 2000 and usage.get("usedWords") == 0 and bool(usage.get("resetsAt")), "New account did not receive the expected unused weekly allowance")
        refreshed = assert_session(api(public, base, "POST", "/auth/v1/token?grant_type=refresh_token", "Refresh session", json={"refresh_token": signed_in["refresh_token"]}), created_id)
        require(refreshed["refresh_token"] != signed_in["refresh_token"], "Refresh token was not rotated")
        print("PASS: password login, wrong-password rejection, refresh rotation, 0/2,000 weekly words")

        recovery_link = api(admin, base, "POST", "/auth/v1/admin/generate_link", "Generate recovery code", json={"type": "recovery", "email": email})
        require(user_id(recovery_link) == created_id, "Recovery code targeted an unexpected account")
        recovered = assert_session(api(public, base, "POST", "/auth/v1/verify", "Verify recovery code", json={"type": "recovery", "email": email, "token": otp(recovery_link)}), created_id)
        updated = api(public, base, "PUT", "/auth/v1/user", "Set recovered password", headers={"Authorization": "Bearer " + recovered["access_token"]}, json={"password": new_password})
        require(user_id(updated) == created_id, "Password update returned an unexpected account")
        old_password = api(public, base, "POST", "/auth/v1/token?grant_type=password", "Reject old password", expected=(400, 401, 403), json={"email": email, "password": password})
        require(old_password.get("error_code") == "invalid_credentials", "Old password remained valid after recovery")
        final_session = assert_session(api(public, base, "POST", "/auth/v1/token?grant_type=password", "Login with recovered password", json={"email": email, "password": new_password}), created_id)
        print("PASS: recovery code changes password; old password fails and new password succeeds")

        api(public, base, "POST", "/auth/v1/logout?scope=local", "Revoke current session", expected=(200, 204), headers={"Authorization": "Bearer " + final_session["access_token"]})
        revoked = api(public, base, "POST", "/auth/v1/token?grant_type=refresh_token", "Reject revoked refresh token", expected=(400, 401, 403), json={"refresh_token": final_session["refresh_token"]})
        require(not revoked.get("access_token"), "Signed-out session could still refresh")
        print("PASS: logout prevents refreshing that session")
    finally:
        if created_id is not None:
            try:
                current = api(admin, base, "GET", "/auth/v1/admin/users/" + created_id, "Verify disposable account before cleanup")
                require(user_id(current) == created_id and current.get("user", current).get("email") == email, "Cleanup refused because account identity did not match")
                api(admin, base, "DELETE", "/auth/v1/admin/users/" + created_id, "Delete disposable account", expected=(200, 204))
                api(admin, base, "GET", "/auth/v1/admin/users/" + created_id, "Verify account deletion", expected=(404,))
                print("CLEANUP: deleted only the newly created synthetic account and verified its removal")
            except Exception:
                print(f"CLEANUP REQUIRED: temporary account ID {created_id}", file=sys.stderr)
                raise
        admin.close()
        public.close()
    print("LIMIT: SMTP delivery, public signup email sending, Google consent, and native UI/Keychain are not covered")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--live", action="store_true", help="Explicitly authorize creating and deleting a disposable test account.")
    parser.add_argument("--project-ref", required=True, help="Explicit Supabase project reference to test.")
    args = parser.parse_args()
    if not args.live or not re.fullmatch(r"[a-z]{20}", args.project_ref):
        parser.error("Use --live and an explicit 20-letter Supabase project reference.")
    try:
        run(args.project_ref)
    except TestFailure as error:
        print(f"FAIL: {error}", file=sys.stderr)
        return 1
    except Exception:
        # Never dump response bodies, traceback locals, keys, OTPs, or passwords.
        print("FAIL: unexpected test harness error; no credentials were printed", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
