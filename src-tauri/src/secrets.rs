//! Almacén de secretos (API keys) del sistema: Llavero en macOS, Administrador de credenciales en
//! Windows y Secret Service en Linux, a través del crate `keyring`. Una entrada por proveedor:
//! servicio = `KEYCHAIN_SERVICE`, cuenta = id del proveedor.
//!
//! Nota: este archivo no estaba en el repositorio original porque `.gitignore` ignoraba `secrets*`;
//! se reconstruyó a partir de su uso en `state.rs` y `commands.rs`.

#[derive(Debug, Clone, thiserror::Error)]
#[error("{0}")]
pub struct SecretError(pub String);

pub trait SecretStore: Send + Sync {
    /// Devuelve el secreto guardado bajo `id` (`None` si no hay ninguno).
    fn get(&self, id: &str) -> Result<Option<String>, SecretError>;
    fn set(&self, id: &str, value: &str) -> Result<(), SecretError>;
    /// Elimina el secreto; que no exista no es un error.
    fn delete(&self, id: &str) -> Result<(), SecretError>;
}

/// Implementación sobre el almacén nativo de cada sistema.
pub struct KeyringSecretStore {
    service: String,
}

impl KeyringSecretStore {
    pub fn new(service: &str) -> Self {
        Self { service: service.to_string() }
    }

    fn entry(&self, id: &str) -> Result<keyring::Entry, SecretError> {
        keyring::Entry::new(&self.service, id).map_err(|e| SecretError(e.to_string()))
    }
}

impl SecretStore for KeyringSecretStore {
    fn get(&self, id: &str) -> Result<Option<String>, SecretError> {
        match self.entry(id)?.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(SecretError(e.to_string())),
        }
    }

    fn set(&self, id: &str, value: &str) -> Result<(), SecretError> {
        self.entry(id)?.set_password(value).map_err(|e| SecretError(e.to_string()))
    }

    fn delete(&self, id: &str) -> Result<(), SecretError> {
        match self.entry(id)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(SecretError(e.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Usa el almacén real del sistema con un servicio de prueba (y lo deja limpio).
    /// Se ejecuta solo con `DICTAMELO_KEYRING_TESTS=1`.
    #[test]
    fn roundtrip_in_system_store() {
        if std::env::var("DICTAMELO_KEYRING_TESTS").is_err() {
            eprintln!("omitido: define DICTAMELO_KEYRING_TESTS=1");
            return;
        }
        let store = KeyringSecretStore::new("com.dictamelo.desktop.tests");
        let id = format!("prueba-{}", uuid::Uuid::new_v4());
        assert_eq!(store.get(&id).unwrap(), None);
        store.set(&id, "valor secreto").unwrap();
        assert_eq!(store.get(&id).unwrap().as_deref(), Some("valor secreto"));
        store.set(&id, "otro valor").unwrap();
        assert_eq!(store.get(&id).unwrap().as_deref(), Some("otro valor"));
        store.delete(&id).unwrap();
        assert_eq!(store.get(&id).unwrap(), None);
        // Borrar dos veces no falla.
        store.delete(&id).unwrap();
    }
}
