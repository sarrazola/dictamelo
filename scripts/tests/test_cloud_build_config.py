"""Public cloud build configuration must reject secrets before a compiler can embed them."""
import base64
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

SCRIPT = Path(__file__).resolve().parents[1] / "with-cloud-config.py"
SPEC = importlib.util.spec_from_file_location("cloud_build_config", SCRIPT)
CONFIG = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CONFIG)


def jwt(role):
    payload = base64.urlsafe_b64encode(json.dumps({"role": role}).encode()).decode().rstrip("=")
    return f"e30.{payload}.c2ln"


def valid_values():
    return {
        "DICTAMELO_SUPABASE_URL": "https://example.supabase.co/",
        "DICTAMELO_SUPABASE_ANON_KEY": jwt("anon"),
        "DICTAMELO_LEMON_STORE_ID": "12",
        "DICTAMELO_LEMON_PRODUCT_ID": "34",
        "DICTAMELO_LEMON_VARIANT_IDS": "57, 56,57",
        "DICTAMELO_CHECKOUT_URL": "https://example.lemonsqueezy.com/buy/example",
    }


class CloudBuildConfigTests(unittest.TestCase):
    def test_public_configuration_normalizes_ids_and_defaults_trial_off(self):
        values = CONFIG.validate_config(valid_values())
        self.assertEqual(values["DICTAMELO_LEMON_VARIANT_IDS"], "56,57")
        self.assertEqual(values["DICTAMELO_BACKEND_URL"], "https://example.supabase.co/functions/v1")
        self.assertEqual(values["DICTAMELO_PRO_TRIAL_AVAILABLE"], "false")
        self.assertEqual(values["DICTAMELO_UPDATES_ENABLED"], "false")
        opted_in = valid_values()
        opted_in["DICTAMELO_UPDATES_ENABLED"] = "true"
        self.assertEqual(CONFIG.validate_config(opted_in)["DICTAMELO_UPDATES_ENABLED"], "true")
        self.assertEqual(CONFIG.public_anon_key("sb_publishable_example_public_key"), "sb_publishable_example_public_key")

    def test_privileged_and_malformed_keys_are_rejected_without_echoing_values(self):
        for secret in (jwt("service_role"), "sb_secret_never_compile_this", "provider-secret-value", "abc.not-base64.secret", "a.W10.c"):
            with self.subTest(kind=secret[:3]):
                with self.assertRaises(CONFIG.ConfigError) as caught:
                    CONFIG.public_anon_key(secret)
                self.assertNotIn(secret, str(caught.exception))

    def test_unsafe_or_malformed_urls_are_rejected(self):
        for field, value in (
            ("DICTAMELO_SUPABASE_URL", "https://name:password@example.com"),
            ("DICTAMELO_SUPABASE_URL", "http://public.example.com"),
            ("DICTAMELO_SUPABASE_URL", "https://example.com/auth"),
            ("DICTAMELO_SUPABASE_URL", "https://example.com:bad"),
            ("DICTAMELO_BACKEND_URL", "https://example.com/api?secret=value"),
            ("DICTAMELO_CHECKOUT_URL", "https://example.lemonsqueezy.com/"),
            ("DICTAMELO_CHECKOUT_URL", "javascript:alert(1)"),
            ("DICTAMELO_CHECKOUT_URL", "https://name:password@example.com/buy/test"),
        ):
            values = valid_values()
            values[field] = value
            with self.subTest(field=field, value=value):
                with self.assertRaises(CONFIG.ConfigError):
                    CONFIG.validate_config(values)
        values = valid_values()
        values["DICTAMELO_SUPABASE_URL"] = "http://127.0.0.1:54321"
        self.assertEqual(CONFIG.validate_config(values)["DICTAMELO_SUPABASE_URL"], values["DICTAMELO_SUPABASE_URL"])

    def test_ambiguous_unknown_or_incomplete_configuration_is_rejected(self):
        for content in (
            "DICTAMELO_LEMON_STORE_ID=12\nDICTAMELO_LEMON_STORE_ID=34",
            "GROQ_API_KEY=never-read-this-as-public-config",
            "export DICTAMELO_LEMON_STORE_ID=12",
            'DICTAMELO_LEMON_STORE_ID="12',
        ):
            with self.assertRaises(CONFIG.ConfigError):
                CONFIG.parse_config(content)
        for field, value in (("DICTAMELO_LEMON_VARIANT_IDS", "56,"), ("DICTAMELO_LEMON_STORE_ID", "0"), ("DICTAMELO_LEMON_PRODUCT_ID", "-1"), ("DICTAMELO_PRO_TRIAL_AVAILABLE", "yes"), ("DICTAMELO_UPDATES_ENABLED", "yes")):
            values = valid_values()
            values[field] = value
            with self.assertRaises(CONFIG.ConfigError):
                CONFIG.validate_config(values)
        values = valid_values()
        del values["DICTAMELO_CHECKOUT_URL"]
        with self.assertRaises(CONFIG.ConfigError):
            CONFIG.validate_config(values)

    def test_secret_configuration_never_launches_requested_command(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory)
            marker = path / "command-ran"
            values = valid_values()
            values["DICTAMELO_SUPABASE_ANON_KEY"] = jwt("service_role")
            config = path / "public-build-config"
            config.write_text("\n".join(f"{key}={value}" for key, value in values.items()))
            result = subprocess.run([sys.executable, str(SCRIPT), "--config", str(config), "--", sys.executable, "-c", "import pathlib,sys;pathlib.Path(sys.argv[1]).touch()", str(marker)], capture_output=True, text=True)
            self.assertEqual(result.returncode, 2)
            self.assertFalse(marker.exists())
            self.assertNotIn(values["DICTAMELO_SUPABASE_ANON_KEY"], result.stdout + result.stderr)

    def test_shell_substitution_is_literal_and_never_executed(self):
        with tempfile.TemporaryDirectory() as directory:
            marker = Path(directory) / "not-created"
            value = f"$(touch {marker})"
            parsed = CONFIG.parse_config(f"DICTAMELO_SUPABASE_URL='{value}'")
            self.assertEqual(parsed["DICTAMELO_SUPABASE_URL"], value)
            values = valid_values()
            values.update(parsed)
            with self.assertRaises(CONFIG.ConfigError):
                CONFIG.validate_config(values)
            self.assertFalse(marker.exists())

    def test_valid_config_executes_literal_arguments_with_explicit_defaults(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory)
            config = path / "public-build-config"
            config.write_text("\n".join(f"{key}={value}" for key, value in valid_values().items()))
            marker = path / "not-created"
            literal = f"$(touch {marker})"
            environment = dict(os.environ, DICTAMELO_PRO_TRIAL_AVAILABLE="true", DICTAMELO_UPDATES_ENABLED="true")
            command = "import json,os,sys;print(json.dumps([os.environ['DICTAMELO_PRO_TRIAL_AVAILABLE'],os.environ['DICTAMELO_UPDATES_ENABLED'],os.environ['DICTAMELO_BACKEND_URL'],sys.argv[1]]))"
            result = subprocess.run([sys.executable, str(SCRIPT), "--config", str(config), "--", sys.executable, "-c", command, literal], env=environment, capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(json.loads(result.stdout), ["false", "false", "https://example.supabase.co/functions/v1", literal])
            self.assertFalse(marker.exists())


if __name__ == "__main__":
    unittest.main()
