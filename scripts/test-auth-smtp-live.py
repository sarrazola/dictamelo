#!/usr/bin/env python3
"""Opt-in real signup and password-recovery email test for an owned mailbox.

Requires requests, an authenticated Supabase CLI, and a terminal. The mailbox must
support plus addressing. Only a freshly generated alias/account is modified. Enter
the actual codes received in that mailbox; never use admin-generated test tokens.
Secrets remain in process memory and code input is hidden. This is not a native UI
test. Mailbox placement and SPF/DKIM/DMARC must be inspected separately.
"""
from __future__ import annotations

import argparse
import getpass
from pathlib import Path
import re
import runpy
import secrets
import sys
import uuid

import requests


helpers = runpy.run_path(str(Path(__file__).with_name("test-auth-live.py")))
TestFailure = helpers["TestFailure"]
require = helpers["require"]
api = helpers["api"]
get_keys = helpers["get_keys"]
user_id = helpers["user_id"]
assert_session = helpers["assert_session"]


def received_code(stage: str) -> str:
    value = getpass.getpass(f"{stage} code from the actual received email: ").strip()
    require(bool(re.fullmatch(r"[0-9]{6,10}", value)), "Expected a numeric email code")
    return value


def run(project: str, mailbox: str) -> None:
    local, domain = mailbox.rsplit("@", 1)
    email = f"{local}+dictamelo-smtp-{uuid.uuid4().hex[:16]}@{domain}"
    password = "Aa1!" + secrets.token_urlsafe(32)
    new_password = "Bb2!" + secrets.token_urlsafe(32)
    base = f"https://{project}.supabase.co"
    anon, service = get_keys(project)
    admin = requests.Session()
    admin.headers.update(apikey=service, Authorization="Bearer " + service)
    public = requests.Session()
    public.headers.update(apikey=anon)
    created_id = None
    signup_attempted = False
    try:
        settings = api(public, base, "GET", "/auth/v1/settings", "Read auth settings")
        require(settings.get("mailer_autoconfirm") is False, "Email confirmations must stay enabled")
        print(f"TEST MAILBOX: {email}", flush=True)
        signup_attempted = True
        signup = api(public, base, "POST", "/auth/v1/signup", "Send public signup email", json={"email": email, "password": password})
        created_id = user_id(signup)
        require(signup.get("user", signup).get("email") == email, "Signup returned an unexpected account")
        require(not signup.get("access_token"), "Signup issued a session before confirmation")
        before = api(public, base, "POST", "/auth/v1/token?grant_type=password", "Reject unconfirmed login", expected=(400, 401, 403), json={"email": email, "password": password})
        require(before.get("error_code") == "email_not_confirmed", "Unconfirmed account did not require confirmation")
        confirmation_code = received_code("SIGNUP")
        confirmed = assert_session(api(public, base, "POST", "/auth/v1/verify", "Verify delivered signup code", json={"type": "signup", "email": email, "token": confirmation_code}), created_id)
        require(bool(confirmed["user"].get("email_confirmed_at")), "Email was not confirmed")
        replay = api(public, base, "POST", "/auth/v1/verify", "Reject signup code replay", expected=(400, 401, 403, 422), json={"type": "signup", "email": email, "token": confirmation_code})
        require(not replay.get("access_token"), "Used code issued another session")
        assert_session(api(public, base, "POST", "/auth/v1/token?grant_type=password", "Login after confirmation", json={"email": email, "password": password}), created_id)
        print("PASS: real signup email/code, confirmation required, code replay rejected, password login", flush=True)

        api(public, base, "POST", "/auth/v1/recover", "Send recovery email", json={"email": email})
        recovery_code = received_code("RECOVERY")
        recovered = assert_session(api(public, base, "POST", "/auth/v1/verify", "Verify delivered recovery code", json={"type": "recovery", "email": email, "token": recovery_code}), created_id)
        updated = api(public, base, "PUT", "/auth/v1/user", "Set recovered password", headers={"Authorization": "Bearer " + recovered["access_token"]}, json={"password": new_password})
        require(user_id(updated) == created_id, "Password update targeted an unexpected account")
        old = api(public, base, "POST", "/auth/v1/token?grant_type=password", "Reject old password", expected=(400, 401, 403), json={"email": email, "password": password})
        require(old.get("error_code") == "invalid_credentials", "Old password still works")
        assert_session(api(public, base, "POST", "/auth/v1/token?grant_type=password", "Login with recovered password", json={"email": email, "password": new_password}), created_id)
        print("PASS: real recovery email/code, password changed, old password rejected, new password accepted", flush=True)
    finally:
        try:
            if created_id is not None:
                try:
                    current = api(admin, base, "GET", "/auth/v1/admin/users/" + created_id, "Check disposable identity before cleanup")
                    require(user_id(current) == created_id and current.get("user", current).get("email") == email, "Cleanup identity check failed")
                    api(admin, base, "DELETE", "/auth/v1/admin/users/" + created_id, "Remove only the disposable account", expected=(200, 204))
                    api(admin, base, "GET", "/auth/v1/admin/users/" + created_id, "Verify account removal", expected=(404,))
                    print("CLEANUP: temporary alias/account removed and deletion verified", flush=True)
                except Exception:
                    print(f"CLEANUP CHECK REQUIRED: temporary account {created_id} at {email}", file=sys.stderr)
                    raise
            elif signup_attempted:
                # A timeout or delivery error can occur after the account was stored.
                # Without a verified ID, report the alias instead of deleting by guess.
                print(f"CLEANUP CHECK REQUIRED: inspect the unique alias {email}; signup returned no verified ID", file=sys.stderr)
        finally:
            admin.close()
            public.close()
    print("LIMIT: public Auth API and delivered codes tested; native UI and mailbox authentication require separate evidence")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--live", action="store_true", help="Authorize two real auth emails and temporary-account cleanup.")
    parser.add_argument("--project-ref", required=True)
    parser.add_argument("--mailbox", required=True, help="Owned mailbox that supports plus aliases; never an unrelated recipient.")
    args = parser.parse_args()
    if not args.live or not re.fullmatch(r"[a-z]{20}", args.project_ref):
        parser.error("Use --live and an explicit 20-letter project reference.")
    if not re.fullmatch(r"[A-Za-z0-9._-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}", args.mailbox):
        parser.error("Use the base address of an owned mailbox with plus-address support.")
    if not sys.stdin.isatty():
        parser.error("Run in a terminal so confirmation and recovery codes are not echoed.")
    try:
        run(args.project_ref, args.mailbox)
    except (KeyboardInterrupt, EOFError):
        print("STOPPED: test interrupted; review cleanup output", file=sys.stderr)
        return 1
    except TestFailure as error:
        print(f"FAIL: {error}", file=sys.stderr)
        return 1
    except Exception:
        print("FAIL: unexpected harness error; no credentials were printed", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
