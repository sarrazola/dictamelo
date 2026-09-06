#!/usr/bin/env python3
"""Run all backend migrations/regressions in a disposable local PostgreSQL cluster.

Requires PostgreSQL server binaries, never uses a linked Supabase project or credentials.
The server listens on a private temporary Unix socket only and is stopped in finally.
"""
import argparse
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
import shutil
import subprocess
import tempfile
import uuid

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--postgres-bin", type=Path)
    args = parser.parse_args()
    binary_dir = args.postgres_bin
    if binary_dir is None:
        found = shutil.which("initdb")
        if found:
            binary_dir = Path(found).resolve().parent
        else:
            binary_dir = Path("/opt/homebrew/opt/postgresql@14/bin")
    if not (binary_dir / "initdb").is_file():
        parser.error("Install PostgreSQL server binaries or pass --postgres-bin.")
    with tempfile.TemporaryDirectory(prefix="dictamelo-free-sql-") as work:
        directory = Path(work)
        data = directory / "data"
        # macOS Unix socket paths are short; /tmp avoids its long per-user TMPDIR prefix.
        with tempfile.TemporaryDirectory(prefix="dictamelo-sql-", dir="/tmp") as socket_path:
            base = [str(binary_dir / "psql"), "-X", "-v", "ON_ERROR_STOP=1", "-U", "postgres",
                    "-d", "postgres", "-h", socket_path, "-p", "55484"]

            def run(argv, check=True):
                return subprocess.run(argv, capture_output=True, text=True, check=check)

            def sql(statement, check=True):
                return run(base + ["-At", "-c", statement], check)

            run([str(binary_dir / "initdb"), "-D", str(data), "-U", "postgres",
                 "--auth-local=trust", "--auth-host=reject"])
            started = False
            try:
                run([str(binary_dir / "pg_ctl"), "-D", str(data), "-l", str(directory / "server.log"),
                     "-o", f"-k {socket_path} -p 55484 -c listen_addresses=''", "-w", "start"])
                started = True
                sql("CREATE ROLE anon; CREATE ROLE authenticated; CREATE ROLE service_role; "
                    "CREATE SCHEMA auth; CREATE TABLE auth.users(id uuid PRIMARY KEY, email text);")
                for path in sorted((ROOT / "supabase/migrations").glob("*.sql")):
                    run(base + ["-f", str(path)])
                    print(f"PASS migration {path.name}")
                for path in sorted((ROOT / "supabase/tests").glob("*.sql")):
                    run(base + ["-f", str(path)])
                    print(f"PASS regression {path.name}")

                user, receipt_one, receipt_two = (str(uuid.uuid4()) for _ in range(3))
                digest = "a" * 64
                sql(f"INSERT INTO auth.users VALUES ('{user}', 'concurrency@example.invalid'); "
                    f"SELECT public.reserve_free_usage('{user}','{receipt_one}'); "
                    f"SELECT public.finish_free_transcription('{user}','{receipt_one}',1,'{digest}'); "
                    "INSERT INTO public.free_weekly_usage(user_id,week_start,words) VALUES "
                    f"('{user}',date_trunc('week',now() AT TIME ZONE 'UTC')::date-7,1); "
                    "INSERT INTO public.free_cleanup_receipts(receipt_id,user_id,week_start,transcript_hash,words) VALUES "
                    f"('{receipt_two}','{user}',date_trunc('week',now() AT TIME ZONE 'UTC')::date-7,'{digest}',1);")

                def claim(receipt):
                    request = str(uuid.uuid4())
                    return sql(f"BEGIN; SELECT public.reserve_free_cleanup('{user}','{receipt}',"
                               f"'{request}','{digest}',1000,1024); SELECT pg_sleep(0.25); COMMIT;", False)

                with ThreadPoolExecutor(max_workers=2) as pool:
                    results = list(pool.map(claim, [receipt_one, receipt_two]))
                assert sum(result.returncode == 0 for result in results) == 1, "Concurrent claims both succeeded or failed"
                assert sum("request_in_progress" in result.stderr for result in results) == 1, "Expected atomic account lock rejection"
                assert sql(f"SELECT public.free_usage('{user}')->>'usedWords'").stdout.strip() == "1", "Cleanup changed word accounting"
                print("PASS two-connection cleanup race across different quota weeks: exactly one claim")
            except subprocess.CalledProcessError as error:
                raise RuntimeError(error.stderr.strip() or "Local PostgreSQL command failed") from None
            finally:
                if started:
                    run([str(binary_dir / "pg_ctl"), "-D", str(data), "-m", "immediate", "-w", "stop"])
    print("PASS isolated cluster stopped and removed; no hosted database accessed")


if __name__ == "__main__":
    main()
