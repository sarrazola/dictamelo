#!/usr/bin/env python3
"""Validate public hosted-build metadata, then run a command without a shell."""
from __future__ import annotations

import argparse
import base64
import binascii
import ipaddress
import json
import os
from pathlib import Path
import re
import sys
from urllib.parse import urlsplit

REQUIRED = {
    "DICTAMELO_SUPABASE_URL",
    "DICTAMELO_SUPABASE_ANON_KEY",
    "DICTAMELO_LEMON_STORE_ID",
    "DICTAMELO_LEMON_PRODUCT_ID",
    "DICTAMELO_LEMON_VARIANT_IDS",
    "DICTAMELO_CHECKOUT_URL",
}
OPTIONAL = {"DICTAMELO_BACKEND_URL", "DICTAMELO_PRO_TRIAL_AVAILABLE", "DICTAMELO_UPDATES_ENABLED"}
ALLOWED = REQUIRED | OPTIONAL


class ConfigError(ValueError):
    """An actionable error that never includes configuration values."""


def parse_config(text: str) -> dict[str, str]:
    values: dict[str, str] = {}
    for number, raw in enumerate(text.splitlines(), 1):
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        if "\x00" in line or "=" not in line:
            raise ConfigError(f"Invalid configuration syntax on line {number}; use KEY=value.")
        name, value = (part.strip() for part in line.split("=", 1))
        if name not in ALLOWED:
            raise ConfigError(f"Unknown field on line {number}; only the documented public build fields are allowed.")
        if name in values:
            raise ConfigError(f"Duplicate field {name} on line {number}.")
        if value.startswith(("'", '"')):
            if len(value) < 2 or value[-1] != value[0]:
                raise ConfigError(f"Unclosed quoted value on line {number}.")
            value = value[1:-1]
        # Quotes delimit a literal value. Shell syntax and variable expansion are never run.
        values[name] = value
    return values


def endpoint(value: str, name: str, *, allow_path: bool, checkout: bool = False) -> str:
    error = f"{name} must be a valid HTTPS URL without credentials or fragments."
    if not value or "\\" in value or any(character.isspace() or ord(character) < 32 for character in value):
        raise ConfigError(error)
    try:
        parsed = urlsplit(value)
        host = parsed.hostname
        _ = parsed.port  # Reject malformed ports before passing the URL to the compiler.
        local = host == "localhost"
        if host and not local:
            try:
                address = ipaddress.ip_address(host)
                local = address.version == 4 and address.is_loopback
            except ValueError:
                pass
        secure = parsed.scheme == "https" or (not checkout and parsed.scheme == "http" and local)
        if not secure or not host or parsed.username is not None or parsed.password is not None or parsed.fragment:
            raise ConfigError(error)
        if not checkout and parsed.query:
            raise ConfigError(f"{name} must not contain query parameters.")
        if not allow_path and parsed.path not in ("", "/"):
            raise ConfigError(f"{name} must be the Supabase project origin, without an additional path.")
        if checkout and parsed.path in ("", "/"):
            raise ConfigError("DICTAMELO_CHECKOUT_URL must point to a checkout, not a store homepage.")
    except (ValueError, UnicodeError) as exc:
        if isinstance(exc, ConfigError):
            raise
        raise ConfigError(error) from None
    return value.rstrip("/") if not checkout else value


def public_anon_key(value: str) -> str:
    error = "DICTAMELO_SUPABASE_ANON_KEY must be a public anon or publishable key; service-role and secret keys are forbidden."
    if value.startswith("sb_publishable_") and len(value) > 20 and re.fullmatch(r"[A-Za-z0-9_-]+", value):
        return value
    pieces = value.split(".")
    if len(pieces) != 3 or not all(re.fullmatch(r"[A-Za-z0-9_-]+", piece) for piece in pieces):
        raise ConfigError(error)
    try:
        payload = pieces[1] + "=" * (-len(pieces[1]) % 4)
        decoded = json.loads(base64.b64decode(payload, altchars=b"-_", validate=True))
    except (ValueError, UnicodeError, binascii.Error):
        raise ConfigError(error) from None
    if not isinstance(decoded, dict) or decoded.get("role") != "anon":
        raise ConfigError(error)
    return value


def positive_id(value: str, name: str) -> str:
    if not re.fullmatch(r"[0-9]{1,20}", value) or not 0 < int(value) <= (2**64 - 1):
        raise ConfigError(f"{name} must contain positive numeric Lemon Squeezy IDs.")
    return str(int(value))


def validate_config(values: dict[str, str]) -> dict[str, str]:
    if set(values) - ALLOWED:
        raise ConfigError("Only documented public build fields may be supplied.")
    missing = sorted(name for name in REQUIRED if not values.get(name, "").strip())
    if missing:
        raise ConfigError("Missing required public build fields: " + ", ".join(missing))
    validated = {name: value.strip() for name, value in values.items()}
    validated["DICTAMELO_SUPABASE_URL"] = endpoint(validated["DICTAMELO_SUPABASE_URL"], "DICTAMELO_SUPABASE_URL", allow_path=False)
    validated["DICTAMELO_SUPABASE_ANON_KEY"] = public_anon_key(validated["DICTAMELO_SUPABASE_ANON_KEY"])
    for name in ("DICTAMELO_LEMON_STORE_ID", "DICTAMELO_LEMON_PRODUCT_ID"):
        validated[name] = positive_id(validated[name], name)
    variants = {positive_id(value.strip(), "DICTAMELO_LEMON_VARIANT_IDS") for value in validated["DICTAMELO_LEMON_VARIANT_IDS"].split(",")}
    validated["DICTAMELO_LEMON_VARIANT_IDS"] = ",".join(sorted(variants, key=int))
    validated["DICTAMELO_CHECKOUT_URL"] = endpoint(validated["DICTAMELO_CHECKOUT_URL"], "DICTAMELO_CHECKOUT_URL", allow_path=True, checkout=True)
    backend = validated.get("DICTAMELO_BACKEND_URL") or validated["DICTAMELO_SUPABASE_URL"] + "/functions/v1"
    validated["DICTAMELO_BACKEND_URL"] = endpoint(backend, "DICTAMELO_BACKEND_URL", allow_path=True)
    trial = validated.get("DICTAMELO_PRO_TRIAL_AVAILABLE", "false")
    if trial not in ("false", "true"):
        raise ConfigError("DICTAMELO_PRO_TRIAL_AVAILABLE must be true or false; enable it only after verifying checkout's trial.")
    validated["DICTAMELO_PRO_TRIAL_AVAILABLE"] = trial
    updates = validated.get("DICTAMELO_UPDATES_ENABLED", "false")
    if updates not in ("false", "true"):
        raise ConfigError("DICTAMELO_UPDATES_ENABLED must be true or false; enable it only for a deliberately configured updater.")
    validated["DICTAMELO_UPDATES_ENABLED"] = updates
    return validated


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--config", type=Path, default=Path(".env.cloud-build"), help="Public build metadata file (default: .env.cloud-build).")
    parser.add_argument("command", nargs=argparse.REMAINDER, help="Command to run after --.")
    args = parser.parse_args(argv)
    command = args.command
    if not command or command[0] != "--" or len(command) < 2:
        parser.error("Supply a command after --, for example: -- cargo test --manifest-path src-tauri/Cargo.toml")
    try:
        values = validate_config(parse_config(args.config.read_text(encoding="utf-8")))
    except (OSError, UnicodeError):
        print("Cloud build configuration could not be read. Copy and fill .env.cloud-build.example first.", file=sys.stderr)
        return 2
    except ConfigError as exc:
        print(f"Cloud build configuration rejected: {exc}", file=sys.stderr)
        return 2
    environment = os.environ.copy()
    environment.update(values)
    try:
        os.execvpe(command[1], command[1:], environment)
    except OSError:
        print("The requested build command could not be started.", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
