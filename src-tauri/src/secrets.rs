//! Cached runtime credentials in the system's secure store, with silent migration from the
//! original service. macOS Keychain access never displays an authentication prompt. Credentials
//! remain encrypted in the system store; release-signing material is excluded entirely.

#[derive(Debug, Clone, thiserror::Error)]
#[error("{0}")]
pub struct SecretError(pub String);

pub trait SecretStore: Send + Sync {
    /// Return the credential stored under `id`, or `None` when absent.
    fn get(&self, id: &str) -> Result<Option<String>, SecretError>;
    fn set(&self, id: &str, value: &str) -> Result<(), SecretError>;
    /// Delete the credential; absence is not an error.
    fn delete(&self, id: &str) -> Result<(), SecretError>;
}

/// A cached runtime-only store. The previous service remains a silent migration source;
/// release-signing material is excluded from this interface and never copied or modified.
pub struct KeyringSecretStore {
    current: std::sync::Arc<dyn SecretStore>,
    legacy: Option<std::sync::Arc<dyn SecretStore>>,
    cache: std::sync::Mutex<std::collections::HashMap<String, CachedSecret>>,
}

enum CachedSecret {
    Value(zeroize::Zeroizing<String>),
    Missing,
    Unavailable(SecretError),
}

impl CachedSecret {
    fn read(&self) -> Result<Option<String>, SecretError> {
        match self {
            Self::Value(value) => Ok(Some(value.to_string())),
            Self::Missing => Ok(None),
            Self::Unavailable(error) => Err(error.clone()),
        }
    }
}

const LEGACY_ACCESS_MESSAGE: &str =
    "This credential was saved by an older app or tool. Enter it again in this app to continue.";
const STORAGE_ACCESS_MESSAGE: &str =
    "Secure storage is unavailable. Reopen the installed app and try again.";
// A nonempty marker is required: legacy macOS Keychain updates can accept an empty password
// without clearing the previous data. This reserved value contains no credential.
const DELETED_MARKER: &str = "dictamelo-runtime-deleted:v1";

impl KeyringSecretStore {
    pub fn new(service: &str) -> Self {
        Self::with_stores(
            std::sync::Arc::new(NativeSecretStore {
                service: Self::runtime_service(service),
            }),
            // A debug app has a different code identity. It must never claim, read, or migrate
            // production credentials, even if a developer uses official cloud settings.
            if cfg!(debug_assertions) {
                None
            } else {
                Some(std::sync::Arc::new(NativeSecretStore {
                    service: service.into(),
                }))
            },
        )
    }

    fn runtime_service(service: &str) -> String {
        if cfg!(debug_assertions) {
            format!("{service}.runtime.debug.v1")
        } else {
            format!("{service}.runtime.v1")
        }
    }

    fn with_stores(
        current: std::sync::Arc<dyn SecretStore>,
        legacy: Option<std::sync::Arc<dyn SecretStore>>,
    ) -> Self {
        Self {
            current,
            legacy,
            cache: std::sync::Mutex::new(std::collections::HashMap::new()),
        }
    }

    fn runtime_id(id: &str) -> Result<(), SecretError> {
        if id.is_empty() || id.starts_with("updater_") {
            return Err(SecretError(
                "This item is not an application runtime credential.".into(),
            ));
        }
        Ok(())
    }
}

impl SecretStore for KeyringSecretStore {
    fn get(&self, id: &str) -> Result<Option<String>, SecretError> {
        Self::runtime_id(id)?;
        // Hold the lock through a cache miss so parallel UI/status calls perform one OS read.
        let mut cache = self.cache.lock().unwrap_or_else(|error| error.into_inner());
        if let Some(value) = cache.get(id) {
            return value.read();
        }
        let value = match self.current.get(id) {
            // A current tombstone must never resurrect a deleted legacy secret.
            Ok(Some(value)) if value.is_empty() || value == DELETED_MARKER => CachedSecret::Missing,
            Ok(Some(value)) => CachedSecret::Value(zeroize::Zeroizing::new(value)),
            Ok(None) => match self.legacy.as_ref().map(|legacy| legacy.get(id)) {
                Some(Ok(Some(value))) if !value.is_empty() => {
                    let value = zeroize::Zeroizing::new(value);
                    // A failed migration leaves the accessible original intact. It is still
                    // usable for this process, and a later launch may retry the silent copy.
                    let _ = self.current.set(id, value.as_str());
                    CachedSecret::Value(value)
                }
                Some(Err(_)) => {
                    CachedSecret::Unavailable(SecretError(LEGACY_ACCESS_MESSAGE.into()))
                }
                _ => CachedSecret::Missing,
            },
            Err(_) => CachedSecret::Unavailable(SecretError(STORAGE_ACCESS_MESSAGE.into())),
        };
        let result = value.read();
        cache.insert(id.into(), value);
        result
    }

    fn set(&self, id: &str, value: &str) -> Result<(), SecretError> {
        Self::runtime_id(id)?;
        if value.is_empty() || value == DELETED_MARKER {
            return Err(SecretError("The credential is empty or reserved.".into()));
        }
        let mut cache = self.cache.lock().unwrap_or_else(|error| error.into_inner());
        // Do not replace a usable cached value or touch legacy data when the write fails.
        self.current
            .set(id, value)
            .map_err(|_| SecretError(STORAGE_ACCESS_MESSAGE.into()))?;
        cache.insert(
            id.into(),
            CachedSecret::Value(zeroize::Zeroizing::new(value.to_string())),
        );
        Ok(())
    }

    fn delete(&self, id: &str) -> Result<(), SecretError> {
        Self::runtime_id(id)?;
        let mut cache = self.cache.lock().unwrap_or_else(|error| error.into_inner());
        // The encrypted marker records explicit deletion even if an old ACL prevents removing
        // the legacy copy. It contains no credential and suppresses future migration.
        self.current
            .set(id, DELETED_MARKER)
            .map_err(|_| SecretError(STORAGE_ACCESS_MESSAGE.into()))?;
        cache.insert(id.into(), CachedSecret::Missing);
        if let Some(legacy) = &self.legacy {
            let _ = legacy.delete(id);
        }
        Ok(())
    }
}

struct NativeSecretStore {
    service: String,
}

#[cfg(target_os = "macos")]
fn initialize_noninteractive_keychain() -> Result<(), SecretError> {
    use std::sync::OnceLock;
    static POLICY: OnceLock<Result<(), SecretError>> = OnceLock::new();
    POLICY
        .get_or_init(|| {
            // Apple's SecItem.h explicitly says per-query no-authentication-UI flags do not
            // suppress UI for legacy file-based Keychain items. keyring's macOS implementation
            // uses that store. Apply a permanent policy to this process before its first access;
            // never use a disable/restore guard, which could race with another credential call.
            // Other applications, browsers, signing tools and TCC permission prompts are unaffected.
            #[link(name = "Security", kind = "framework")]
            extern "C" {
                fn SecKeychainSetUserInteractionAllowed(state: u8) -> i32;
            }
            let status = unsafe { SecKeychainSetUserInteractionAllowed(0) };
            if status == 0 {
                Ok(())
            } else {
                Err(SecretError(STORAGE_ACCESS_MESSAGE.into()))
            }
        })
        .clone()
}

#[cfg(not(target_os = "macos"))]
fn initialize_noninteractive_keychain() -> Result<(), SecretError> {
    Ok(())
}

impl NativeSecretStore {
    fn entry(&self, id: &str) -> Result<keyring::Entry, SecretError> {
        // If initialization fails, stop before calling an API that could display a prompt.
        initialize_noninteractive_keychain()?;
        keyring::Entry::new(&self.service, id)
            .map_err(|_| SecretError(STORAGE_ACCESS_MESSAGE.into()))
    }
}

impl SecretStore for NativeSecretStore {
    fn get(&self, id: &str) -> Result<Option<String>, SecretError> {
        match self.entry(id)?.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err(SecretError(STORAGE_ACCESS_MESSAGE.into())),
        }
    }

    fn set(&self, id: &str, value: &str) -> Result<(), SecretError> {
        #[cfg(target_os = "macos")]
        {
            initialize_noninteractive_keychain()?;
            // SecItemAdd/Update does not first retrieve the previous secret, unlike the
            // legacy set_generic_password implementation used by keyring's v1 facade.
            security_framework::passwords::set_generic_password(&self.service, id, value.as_bytes())
                .map_err(|_| SecretError(STORAGE_ACCESS_MESSAGE.into()))
        }
        #[cfg(not(target_os = "macos"))]
        self.entry(id)?
            .set_password(value)
            .map_err(|_| SecretError(STORAGE_ACCESS_MESSAGE.into()))
    }

    fn delete(&self, id: &str) -> Result<(), SecretError> {
        match self.entry(id)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err(SecretError(STORAGE_ACCESS_MESSAGE.into())),
        }
    }
}

/// In-memory storage for tests that must not touch the real Keychain.
#[cfg(test)]
#[derive(Default)]
pub struct MemorySecretStore {
    values: std::sync::Mutex<std::collections::HashMap<String, String>>,
}

#[cfg(test)]
impl SecretStore for MemorySecretStore {
    fn get(&self, id: &str) -> Result<Option<String>, SecretError> {
        Ok(self.values.lock().unwrap().get(id).cloned())
    }
    fn set(&self, id: &str, value: &str) -> Result<(), SecretError> {
        self.values.lock().unwrap().insert(id.into(), value.into());
        Ok(())
    }
    fn delete(&self, id: &str) -> Result<(), SecretError> {
        self.values.lock().unwrap().remove(id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    };

    #[derive(Default)]
    struct CountingStore {
        values: MemorySecretStore,
        reads: AtomicUsize,
        writes: AtomicUsize,
        deletes: AtomicUsize,
        deny_read: AtomicBool,
        deny_write: AtomicBool,
        deny_delete: AtomicBool,
    }
    impl SecretStore for CountingStore {
        fn get(&self, id: &str) -> Result<Option<String>, SecretError> {
            self.reads.fetch_add(1, Ordering::SeqCst);
            if self.deny_read.load(Ordering::SeqCst) {
                return Err(SecretError("Access denied".into()));
            }
            self.values.get(id)
        }
        fn set(&self, id: &str, value: &str) -> Result<(), SecretError> {
            self.writes.fetch_add(1, Ordering::SeqCst);
            if self.deny_write.load(Ordering::SeqCst) {
                return Err(SecretError("Write denied".into()));
            }
            self.values.set(id, value)
        }
        fn delete(&self, id: &str) -> Result<(), SecretError> {
            self.deletes.fetch_add(1, Ordering::SeqCst);
            if self.deny_delete.load(Ordering::SeqCst) {
                return Err(SecretError("Delete denied".into()));
            }
            self.values.delete(id)
        }
    }

    #[test]
    fn debug_builds_cannot_share_the_production_namespace_or_migration_source() {
        let store = KeyringSecretStore::new("com.dictamelo.desktop");
        if cfg!(debug_assertions) {
            assert_eq!(
                KeyringSecretStore::runtime_service("com.dictamelo.desktop"),
                "com.dictamelo.desktop.runtime.debug.v1"
            );
            assert!(store.legacy.is_none());
        } else {
            assert_eq!(
                KeyringSecretStore::runtime_service("com.dictamelo.desktop"),
                "com.dictamelo.desktop.runtime.v1"
            );
            assert!(store.legacy.is_some());
        }
    }

    #[test]
    fn concurrent_status_reads_load_and_migrate_once() {
        let current = Arc::new(CountingStore::default());
        let legacy = Arc::new(CountingStore::default());
        legacy.values.set("groq", "test-provider-value").unwrap();
        let store = Arc::new(KeyringSecretStore::with_stores(
            current.clone(),
            Some(legacy.clone()),
        ));
        let handles: Vec<_> = (0..8)
            .map(|_| {
                let store = store.clone();
                std::thread::spawn(move || {
                    assert_eq!(
                        store.get("groq").unwrap().as_deref(),
                        Some("test-provider-value")
                    )
                })
            })
            .collect();
        for handle in handles {
            handle.join().unwrap();
        }
        assert_eq!(current.reads.load(Ordering::SeqCst), 1);
        assert_eq!(legacy.reads.load(Ordering::SeqCst), 1);
        assert_eq!(current.writes.load(Ordering::SeqCst), 1);
        assert_eq!(
            legacy.deletes.load(Ordering::SeqCst),
            0,
            "Migration preserves its original"
        );
        let reopened = KeyringSecretStore::with_stores(current.clone(), Some(legacy.clone()));
        assert_eq!(
            reopened.get("groq").unwrap().as_deref(),
            Some("test-provider-value")
        );
        assert_eq!(
            legacy.reads.load(Ordering::SeqCst),
            1,
            "A migrated item must not reread legacy storage"
        );
    }

    #[test]
    fn inaccessible_legacy_item_is_preserved_until_user_enters_replacement() {
        let current = Arc::new(CountingStore::default());
        let legacy = Arc::new(CountingStore::default());
        legacy.values.set("groq", "old-value").unwrap();
        legacy.deny_read.store(true, Ordering::SeqCst);
        let store = KeyringSecretStore::with_stores(current.clone(), Some(legacy.clone()));
        for _ in 0..3 {
            assert!(store
                .get("groq")
                .unwrap_err()
                .to_string()
                .contains("Enter it again"));
        }
        assert_eq!(
            legacy.reads.load(Ordering::SeqCst),
            1,
            "Do not repeatedly try a denied item"
        );
        assert_eq!(current.writes.load(Ordering::SeqCst), 0);
        assert_eq!(legacy.deletes.load(Ordering::SeqCst), 0);
        store.set("groq", "replacement-value").unwrap();
        assert_eq!(
            store.get("groq").unwrap().as_deref(),
            Some("replacement-value")
        );
        assert_eq!(
            legacy.values.get("groq").unwrap().as_deref(),
            Some("old-value")
        );
    }

    #[test]
    fn failed_save_preserves_the_cached_and_persisted_previous_key() {
        let current = Arc::new(CountingStore::default());
        let store = KeyringSecretStore::with_stores(current.clone(), None);
        store.set("openai", "old-value").unwrap();
        current.deny_write.store(true, Ordering::SeqCst);
        assert!(store.set("openai", "new-value").is_err());
        assert_eq!(store.get("openai").unwrap().as_deref(), Some("old-value"));
        assert_eq!(
            current.values.get("openai").unwrap().as_deref(),
            Some("old-value")
        );
        assert_eq!(
            current.reads.load(Ordering::SeqCst),
            0,
            "Saving and displaying a key must not read it back"
        );
    }

    #[test]
    fn failed_migration_remains_usable_and_failed_deletion_preserves_the_original() {
        let current = Arc::new(CountingStore::default());
        let legacy = Arc::new(CountingStore::default());
        legacy.values.set("openai", "old-value").unwrap();
        current.deny_write.store(true, Ordering::SeqCst);
        let store = KeyringSecretStore::with_stores(current.clone(), Some(legacy.clone()));
        assert_eq!(store.get("openai").unwrap().as_deref(), Some("old-value"));
        assert!(store.delete("openai").is_err());
        assert_eq!(store.get("openai").unwrap().as_deref(), Some("old-value"));
        assert_eq!(
            legacy.values.get("openai").unwrap().as_deref(),
            Some("old-value")
        );
        assert_eq!(legacy.deletes.load(Ordering::SeqCst), 0);
        assert_eq!(legacy.reads.load(Ordering::SeqCst), 1);

        current.deny_write.store(false, Ordering::SeqCst);
        let reopened = KeyringSecretStore::with_stores(current.clone(), Some(legacy.clone()));
        assert_eq!(
            reopened.get("openai").unwrap().as_deref(),
            Some("old-value")
        );
        assert_eq!(
            current.values.get("openai").unwrap().as_deref(),
            Some("old-value")
        );
    }

    #[test]
    fn deletion_tombstone_prevents_legacy_secret_resurrection_after_restart() {
        let current = Arc::new(CountingStore::default());
        let legacy = Arc::new(CountingStore::default());
        legacy
            .values
            .set("supabase_session", "test-session")
            .unwrap();
        legacy.deny_delete.store(true, Ordering::SeqCst);
        let store = KeyringSecretStore::with_stores(current.clone(), Some(legacy.clone()));
        assert!(store.get("supabase_session").unwrap().is_some());
        store.delete("supabase_session").unwrap();
        assert!(store.get("supabase_session").unwrap().is_none());
        let reopened = KeyringSecretStore::with_stores(current.clone(), Some(legacy.clone()));
        assert!(reopened.get("supabase_session").unwrap().is_none());
        assert_eq!(legacy.reads.load(Ordering::SeqCst), 1);
        assert_eq!(
            legacy.values.get("supabase_session").unwrap().as_deref(),
            Some("test-session")
        );
    }

    #[test]
    fn missing_entries_are_cached_and_signing_material_is_never_accessed() {
        let current = Arc::new(CountingStore::default());
        let legacy = Arc::new(CountingStore::default());
        let store = KeyringSecretStore::with_stores(current.clone(), Some(legacy.clone()));
        for _ in 0..3 {
            assert!(store.get("openai").unwrap().is_none());
        }
        assert_eq!(current.reads.load(Ordering::SeqCst), 1);
        assert_eq!(legacy.reads.load(Ordering::SeqCst), 1);
        assert!(store.get("updater_private_key").is_err());
        assert!(store.set("updater_private_key", "not-allowed").is_err());
        assert!(store.delete("updater_private_key").is_err());
        assert_eq!(current.reads.load(Ordering::SeqCst), 1);
        assert_eq!(legacy.reads.load(Ordering::SeqCst), 1);
        assert_eq!(current.writes.load(Ordering::SeqCst), 0);
        assert_eq!(legacy.deletes.load(Ordering::SeqCst), 0);
    }

    /// Use only a unique account in the test service, with cleanup even after a failed assertion.
    /// Run explicitly with `DICTAMELO_KEYRING_TESTS=1`; normal tests never access the OS store.
    #[test]
    fn roundtrip_in_system_store() {
        if std::env::var("DICTAMELO_KEYRING_TESTS").as_deref() != Ok("1") {
            eprintln!("skipped: set DICTAMELO_KEYRING_TESTS=1");
            return;
        }
        const SERVICE: &str = "com.dictamelo.desktop.tests";
        struct Cleanup(String);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let store = KeyringSecretStore::new(SERVICE);
                let _ = store.current.delete(&self.0);
                let _ = NativeSecretStore {
                    service: SERVICE.into(),
                }
                .delete(&self.0);
            }
        }
        let id = Cleanup(format!("test-{}", uuid::Uuid::new_v4()));
        let store = KeyringSecretStore::new(SERVICE);
        assert_eq!(store.get(&id.0).unwrap(), None);
        #[cfg(target_os = "macos")]
        assert!(
            !security_framework::os::macos::keychain::SecKeychain::user_interaction_allowed()
                .unwrap()
        );
        // A debug process cannot import even an accessible item from the original service.
        if cfg!(debug_assertions) {
            NativeSecretStore {
                service: SERVICE.into(),
            }
            .set(&id.0, "synthetic-legacy-value")
            .unwrap();
            assert_eq!(KeyringSecretStore::new(SERVICE).get(&id.0).unwrap(), None);
        }
        store.set(&id.0, "synthetic-test-value").unwrap();
        let reopened = KeyringSecretStore::new(SERVICE);
        assert_eq!(
            reopened.get(&id.0).unwrap().as_deref(),
            Some("synthetic-test-value")
        );
        reopened.set(&id.0, "synthetic-updated-value").unwrap();
        let reopened = KeyringSecretStore::new(SERVICE);
        assert_eq!(
            reopened.get(&id.0).unwrap().as_deref(),
            Some("synthetic-updated-value")
        );
        reopened.delete(&id.0).unwrap();
        let reopened = KeyringSecretStore::new(SERVICE);
        assert_eq!(reopened.get(&id.0).unwrap(), None);
        reopened.delete(&id.0).unwrap();
        reopened.current.delete(&id.0).unwrap();
        assert_eq!(reopened.current.get(&id.0).unwrap(), None);
        let legacy = NativeSecretStore {
            service: SERVICE.into(),
        };
        legacy.delete(&id.0).unwrap();
        assert_eq!(legacy.get(&id.0).unwrap(), None);
        #[cfg(target_os = "macos")]
        assert!(
            !security_framework::os::macos::keychain::SecKeychain::user_interaction_allowed()
                .unwrap()
        );
    }
}
