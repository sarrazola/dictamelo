# Repository maintenance

- Keep public repository documentation, GitHub metadata, release notes, and new developer-facing instructions in English. Preserve the app's six interface languages.
- Keep the app small. Preserve personal API-key mode and existing Pro licenses.
- Never place provider secrets, service-role keys, refresh tokens, private updater keys, or Apple credentials in source files, logs, issues, or release notes. The Supabase client anon key is public by design.
- `src-tauri/src/secrets.rs` is source code and must remain tracked. `.gitignore` excludes private files, not instructions.
- For every release, follow `docs/RELEASING.md`: synchronize all manifest/lockfile versions, update README claims and download architecture guidance, add English release notes and CHANGELOG, run appropriate Rust/backend/UI checks, and verify the actual artifacts.
- Published Windows installers must cover x86_64 (Intel/AMD) and aarch64. Derive architecture from the build target and verify the PE header; never infer it from the machine running the script.
- macOS release builds require Developer ID signing, notarization, stapling and Gatekeeper verification. Package and sign updater archives after stapling.
- Use the existing Tauri private key on all build machines. Preserve all platform entries in `latest.json`. Upload immutable artifacts before publishing the complete manifest.
- Commit explicit paths; never use a release script to stage unrelated changes or push unrelated tags. Never overwrite an already-published version's installers.
- Keep `docs/TESTING.md` and platform reports honest: distinguish unit tests, UI mocks, live backend tests, VM emulation, physical hardware tests, and external setup that is still pending.
