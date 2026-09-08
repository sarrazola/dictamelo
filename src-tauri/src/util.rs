//! Utilidades pequeñas compartidas.

use std::sync::{Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard};

/// Commit a complete sibling file instead of truncating the last valid JSON copy.
/// This protects replacement failures; it is not a power-loss durability guarantee.
pub fn atomic_write(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let parent = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or_else(|| std::path::Path::new("."));
    std::fs::create_dir_all(parent)?;
    let temporary = parent.join(format!(".dictamelo-{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(&temporary)?;
        if let Ok(metadata) = std::fs::metadata(path) {
            file.set_permissions(metadata.permissions())?;
        }
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&temporary, path)
    })();
    if result.is_err() { let _ = std::fs::remove_file(&temporary); }
    result
}

/// Bloquea un `Mutex` ignorando el envenenamiento (un panic en otro hilo no debe tumbar la app).
pub fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn read<T>(l: &RwLock<T>) -> RwLockReadGuard<'_, T> {
    l.read().unwrap_or_else(|e| e.into_inner())
}

pub fn write<T>(l: &RwLock<T>) -> RwLockWriteGuard<'_, T> {
    l.write().unwrap_or_else(|e| e.into_inner())
}

/// Recorta un texto a `max` caracteres añadiendo "…" si hizo falta.
pub fn truncate(text: &str, max: usize) -> String {
    let mut out: String = text.chars().take(max).collect();
    if text.chars().count() > max {
        out.push('…');
    }
    out
}

#[cfg(test)]
mod persistence_tests {
    use super::atomic_write;

    #[test]
    fn replacement_failure_preserves_existing_data_and_removes_temporary_file() {
        let dir = std::env::temp_dir().join(format!("dictamelo-atomic-{}", uuid::Uuid::new_v4()));
        let file = dir.join("settings.json");
        atomic_write(&file, b"old").unwrap();
        atomic_write(&file, b"new").unwrap();
        assert_eq!(std::fs::read(&file).unwrap(), b"new");
        let directory = dir.join("directory");
        std::fs::create_dir(&directory).unwrap();
        std::fs::write(directory.join("original"), b"keep").unwrap();
        assert!(atomic_write(&directory, b"cannot replace directory").is_err());
        assert_eq!(std::fs::read(directory.join("original")).unwrap(), b"keep");
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 2);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn replacement_preserves_restricted_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("dictamelo-permissions-{}", uuid::Uuid::new_v4()));
        let file = dir.join("history.json");
        atomic_write(&file, b"old").unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
        atomic_write(&file, b"new").unwrap();
        assert_eq!(std::fs::metadata(&file).unwrap().permissions().mode() & 0o777, 0o600);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
