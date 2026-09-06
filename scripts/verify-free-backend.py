#!/usr/bin/env python3
"""Compatibility entry point for the comprehensive opt-in Free Cloud regression.

Uses the licensed audio fixture and synthetic accounts with verified deletion.
"""
from pathlib import Path
import runpy
import sys

if sys.argv[1:] != ["--live"]:
    raise SystemExit("Usage: python3 scripts/verify-free-backend.py --live")
sys.argv = ["test-free-cleanup-live.py", "--live", "--project-ref", "iburiyhhfodndqgmsaot"]
runpy.run_path(str(Path(__file__).with_name("test-free-cleanup-live.py")), run_name="__main__")
