# Local credential storage

Reviewed on September 5, 2026. This document explains the 0.3.1 macOS credential change and the alternatives considered. It does not describe a new public release. Native API tests, installed-candidate login checks and final installed-artifact verification are distinguished below.

## Decision

Keep API keys, account sessions, and Pro license credentials in the operating system's secure store. On macOS, use a separate runtime namespace, suppress interactive Keychain access for the application process, and cache each lookup. An inaccessible credential produces an actionable error instead of a password dialog. Do not move credentials into an ordinary SQLite database or settings JSON file.

This is the smaller change for the current app. A file vault encrypted with one Keychain-backed master key remains a possible future design for a larger credential collection; it is **not implemented** by this change.

## Why the previous implementation could prompt repeatedly

The previous [`SecretStore`](../src-tauri/src/secrets.rs) implementation performed a native password read on every `get`. [`get_api_key_status`](../src-tauri/src/commands.rs) used that read merely to report whether a key existed and show its last four characters. Startup, settings changes, and the onboarding save flow could trigger repeated or concurrent status reads. There was no cache.

The macOS backend used by `keyring` 4.2 is the legacy login Keychain. Its access controls can depend on the application's designated code-signing requirement. A credential created by an older unsigned build, a differently signed build, or another tool can require new authorization when the installed app accesses it. Selecting “Allow Once” also permits a later prompt. These are plausible causes; this review did **not** prove a particular ACL mismatch on the user's credentials. [Apple's signing explanation](https://developer.apple.com/library/archive/technotes/tn2206/_index.html) and [Keychain authorization choices](https://support.apple.com/en-ca/guide/keychain-access/kyca1243/mac) describe the mechanisms.

Developer ID signing and notarization remain required for releases. They do not automatically grant access to every pre-existing Keychain item. Keep the official app's bundle identifier and signing identity consistent across updates.

## Current implementation

The implementation is in [`src-tauri/src/secrets.rs`](../src-tauri/src/secrets.rs), shared by provider keys, the Supabase session, and runtime Pro license records.

- **Separate runtime namespaces:** release entries use `com.dictamelo.desktop.runtime.v1`; debug entries use `com.dictamelo.desktop.runtime.debug.v1`. Only release builds have the old `com.dictamelo.desktop` service as a legacy migration source. Debug builds neither read nor import installed-app credentials. Runtime operations reject `updater_*` items; release-signing material is not migrated or modified through this store.
- **Noninteractive macOS access:** before its first native credential operation, the process initializes `SecKeychainSetUserInteractionAllowed(false)` once. It leaves that policy in place to avoid races between simultaneous requests. An initialization failure stops the access attempt. This affects the app's Keychain operations, not other applications or microphone/accessibility permission dialogs. The app does not bypass a denied ACL or unlock a locked Keychain.
- **One lookup per credential per process:** a mutex serializes cache misses. Successful values, missing entries, and unavailable results are cached. Status checks can still request a credential, but subsequent checks use this cache rather than making another OS read. There is no new persisted metadata file.
- **Cache handling:** successful saves update the cache directly after the native write succeeds. Cached values use `zeroize::Zeroizing<String>` so their owned buffers are cleared when dropped or replaced. Callers still receive ordinary string copies needed for requests; this is not a claim that every temporary copy is zeroized.
- **Silent migration in release builds:** when no new entry exists, the store tries the corresponding legacy entry without displaying authorization UI. An accessible value is copied to the new namespace. The original remains intact. If copying fails, the readable value can still be used from memory for that process and migration can retry on a later launch.
- **Recovery:** a denied legacy read asks the user to enter that credential again in the app. Saving writes to the new namespace. A failed save retains the previous cached and persisted value. Unavailable results are not repeatedly retried during status refreshes; a successful replacement or process restart changes that state.
- **Deletion and logout:** the reserved non-secret value `dictamelo-runtime-deleted:v1` in the new Keychain entry records a deletion, then release builds attempt legacy deletion silently. The normal save API rejects that reserved value. This tombstone prevents an inaccessible legacy key or session from being imported again after restart. If the OS refuses legacy deletion, the old item can remain in Keychain, but Dictámelo will not reuse it. An actual macOS test showed that an empty-password update could leave the previous password intact, which is why the implementation uses an explicit marker.

On macOS, native writes use `SecItemAdd`/`SecItemUpdate` via `security-framework`, avoiding a preliminary password read performed by the older setter. The interaction policy targets the legacy Keychain API already used by this backend. Apple distinguishes that store's ACLs from the data protection Keychain's access groups; switching implementations requires a separate migration and signing/entitlement review. [Apple TN3137](https://developer.apple.com/documentation/technotes/tn3137-on-mac-keychains), [Keychain interaction API](https://developer.apple.com/documentation/security/keychains).

## SQLite versus a future encrypted vault

SQLite organizes data; standard SQLite does not encrypt credential values. Moving keys from Keychain to a normal database would trade away at-rest protection without addressing encryption-key ownership. A password-protected database also needs somewhere to keep its password.

If the app later needs a shared vault for many credentials, the useful pattern is authenticated encryption with a random master key kept in the OS secure store and cached in the native process. The encrypted records could live in files or SQLite; that choice is secondary. Such a change would require versioned ciphertext, safe writes, and a verified migration before removing originals. A master key must never be embedded in source or saved beside the encrypted records in plaintext.

For this version, the native store plus noninteractive access, caching, and migration isolation is the chosen implementation. It keeps the app small and avoids introducing a new encryption format or database solely for API keys.

## Verification boundary

The 0.3.1 Rust suite passed 55 tests with `DICTAMELO_KEYRING_TESTS=1`, including eight credential tests; strict Clippy also passed. The native synthetic test checked the noninteractive flag before and after operations, persistence through a fresh store instance, updates, deletion, debug isolation from an accessible legacy test entry, and cleanup of both test namespaces. It used dedicated synthetic credentials. Concurrent reads, silent migration, denied legacy access, failed writes and tombstone behavior also have in-memory regression coverage; a mocked ACL denial is not a real denied-item migration test.

In the signed installed 0.3.1 candidate, real Google sign-in and quit/reopen session persistence succeeded without a Keychain prompt. The app also detected the existing personal Groq key as stored without prompting. The session remained loaded after replacing the app with the final verified artifact. These observations do not prove every provider's save/delete path, physical dictation, or recovery from every legacy ACL condition. See [Testing](TESTING.md) for the final installed-artifact and native application-menu results. No new Windows verification is claimed.
