//! Lemon Squeezy Pro licenses for an explicitly configured hosted build. License keys,
//! activation instances, and previously verified ownership stay in the OS credential store.
use crate::secrets::SecretStore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::time::Duration;

const API: &str = "https://api.lemonsqueezy.com/v1/licenses";
const KEY_NAME: &str = "license_key";
const INSTANCE_NAME: &str = "license_instance";
const PROOF_NAME: &str = "license_verified_ownership";
const MAX_OFFLINE_SECONDS: i64 = 7 * 24 * 60 * 60;
const NOT_CONFIGURED: &str = "Pro licensing is not configured in this build. Use your own API key or install the official Dictámelo app.";
static GATE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[derive(Debug, Clone, Serialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LicenseStatus {
    pub active: bool,
    pub key_hint: Option<String>,
    pub status: Option<String>,
    pub message: Option<String>,
}

#[derive(Deserialize, Default)]
struct ApiResponse {
    #[serde(default)]
    activated: bool,
    #[serde(default)]
    deactivated: bool,
    #[serde(default)]
    valid: bool,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    license_key: Option<KeyInfo>,
    #[serde(default)]
    instance: Option<InstanceInfo>,
    #[serde(default)]
    meta: Option<Ownership>,
}

#[derive(Deserialize)]
struct KeyInfo {
    status: String,
    #[serde(default)]
    expires_at: Option<String>,
}

#[derive(Deserialize)]
struct InstanceInfo {
    id: String,
}

#[derive(Clone, Deserialize, Serialize, PartialEq)]
struct Ownership {
    store_id: u64,
    product_id: u64,
    variant_id: u64,
}

struct LicenseConfig {
    store_id: u64,
    product_id: u64,
    variant_ids: Vec<u64>,
    project_url: String,
    backend_url: String,
}

impl LicenseConfig {
    fn owns(&self, ownership: &Ownership) -> bool {
        ownership.store_id == self.store_id
            && ownership.product_id == self.product_id
            && self.variant_ids.contains(&ownership.variant_id)
    }

    fn binding(&self) -> String {
        let public_binding = serde_json::json!([
            self.project_url,
            self.backend_url,
            self.store_id,
            self.product_id,
            self.variant_ids
        ]);
        digest(&public_binding.to_string())
    }
}

#[derive(Deserialize, Serialize)]
struct VerifiedLicense {
    key_digest: String,
    instance_id: String,
    config_binding: String,
    ownership: Ownership,
    verified_at: i64,
    expires_at: Option<String>,
}

#[derive(Debug)]
enum CallError {
    Unavailable,
    InvalidResponse,
}
impl CallError {
    fn message(&self) -> &'static str {
        match self {
            Self::Unavailable => {
                "Could not reach the license service. Please try again when connected."
            }
            Self::InvalidResponse => {
                "The license service returned an unexpected response. Please try again."
            }
        }
    }
}

fn id(value: Option<&str>) -> Result<u64, String> {
    value
        .and_then(|v| v.trim().parse::<u64>().ok())
        .filter(|v| *v > 0)
        .ok_or_else(|| NOT_CONFIGURED.into())
}

fn parse_config(
    store: Option<&str>,
    product: Option<&str>,
    variants: Option<&str>,
    project_url: &str,
    backend_url: &str,
) -> Result<LicenseConfig, String> {
    let mut variant_ids = variants
        .ok_or(NOT_CONFIGURED)?
        .split(',')
        .map(|v| id(Some(v)))
        .collect::<Result<Vec<_>, _>>()?;
    if variant_ids.is_empty() {
        return Err(NOT_CONFIGURED.into());
    }
    variant_ids.sort_unstable();
    variant_ids.dedup();
    Ok(LicenseConfig {
        store_id: id(store)?,
        product_id: id(product)?,
        variant_ids,
        project_url: project_url.into(),
        backend_url: backend_url.into(),
    })
}

fn config() -> Result<LicenseConfig, String> {
    if !crate::cloud_config::configured() {
        return Err(NOT_CONFIGURED.into());
    }
    parse_config(
        option_env!("DICTAMELO_LEMON_STORE_ID"),
        option_env!("DICTAMELO_LEMON_PRODUCT_ID"),
        option_env!("DICTAMELO_LEMON_VARIANT_IDS"),
        crate::cloud_config::supabase_url()?,
        crate::cloud_config::backend_url()?,
    )
}

pub fn checkout_url() -> Result<String, String> {
    config()?;
    parse_checkout_url(option_env!("DICTAMELO_CHECKOUT_URL"))
}

fn parse_checkout_url(value: Option<&str>) -> Result<String, String> {
    let error = "The Pro checkout is not configured in this build. Please contact support.";
    let url = reqwest::Url::parse(value.ok_or(error)?.trim()).map_err(|_| error.to_string())?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
        || url.path() == "/"
    {
        return Err(error.into());
    }
    Ok(url.into())
}

fn digest(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

fn hint(key: &str) -> Option<String> {
    let key = key.trim();
    (key.chars().count() > 8).then(|| {
        format!(
            "…{}",
            key.chars()
                .rev()
                .take(4)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect::<String>()
        )
    })
}

async fn call(path: &str, params: &[(&str, &str)]) -> Result<ApiResponse, CallError> {
    let body: serde_json::Map<String, serde_json::Value> = params
        .iter()
        .map(|(k, v)| {
            (
                (*k).to_string(),
                serde_json::Value::String((*v).to_string()),
            )
        })
        .collect();
    let response = crate::transcription::shared_http_client()
        .post(format!("{API}/{path}"))
        .header("Accept", "application/json")
        .timeout(Duration::from_secs(20))
        .json(&body)
        .send()
        .await
        .map_err(|_| CallError::Unavailable)?;
    if response.status().is_server_error() || response.status().as_u16() == 429 {
        return Err(CallError::Unavailable);
    }
    response
        .json()
        .await
        .map_err(|_| CallError::InvalidResponse)
}

fn ownership<'a>(
    response: &'a ApiResponse,
    config: &LicenseConfig,
) -> Result<&'a Ownership, String> {
    response
        .meta
        .as_ref()
        .filter(|meta| config.owns(meta))
        .ok_or_else(|| "This license does not belong to the configured Pro product or plan.".into())
}

fn expires_in_future(expires_at: Option<&str>, now: i64) -> bool {
    match expires_at {
        None => true,
        Some(value) => {
            chrono::DateTime::parse_from_rfc3339(value).is_ok_and(|date| date.timestamp() > now)
        }
    }
}

fn usable_key(response: &ApiResponse, needs_instance: bool) -> bool {
    response.license_key.as_ref().is_some_and(|key| {
        (key.status == "active" || (!needs_instance && key.status == "inactive"))
            && expires_in_future(key.expires_at.as_deref(), chrono::Utc::now().timestamp())
    })
}

fn rejection(response: &ApiResponse) -> String {
    // Only known public messages are shown. Arbitrary responses can contain keys or customer data.
    let message = response
        .error
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    if message.contains("activation limit") {
        "This license has reached its device limit. Deactivate another device first."
    } else if message.contains("expired") {
        "This license has expired."
    } else if message.contains("disabled") {
        "This license has been disabled."
    } else {
        "This license could not be verified. Check the key and your subscription."
    }
    .into()
}

fn proof(secrets: &Arc<dyn SecretStore>) -> Option<VerifiedLicense> {
    secrets
        .get(PROOF_NAME)
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
}

fn proof_valid(
    proof: &VerifiedLicense,
    config: &LicenseConfig,
    key: &str,
    instance: &str,
    now: i64,
) -> bool {
    proof.key_digest == digest(key)
        && proof.instance_id == instance
        && proof.config_binding == config.binding()
        && config.owns(&proof.ownership)
        && proof.verified_at <= now
        && now - proof.verified_at <= MAX_OFFLINE_SECONDS
        && expires_in_future(proof.expires_at.as_deref(), now)
}

fn remember(
    secrets: &Arc<dyn SecretStore>,
    config: &LicenseConfig,
    key: &str,
    instance: &str,
    response: &ApiResponse,
) -> Result<(), String> {
    let ownership = ownership(response, config)?.clone();
    let record = VerifiedLicense {
        key_digest: digest(key),
        instance_id: instance.into(),
        config_binding: config.binding(),
        ownership,
        verified_at: chrono::Utc::now().timestamp(),
        expires_at: response
            .license_key
            .as_ref()
            .and_then(|info| info.expires_at.clone()),
    };
    let value = serde_json::to_string(&record)
        .map_err(|_| "Could not save the verified license.".to_string())?;
    secrets
        .set(PROOF_NAME, &value)
        .map_err(|_| "Could not save the verified license in the system credential store.".into())
}

fn active_status(key: &str, message: Option<String>) -> LicenseStatus {
    LicenseStatus {
        active: true,
        key_hint: hint(key),
        status: Some("active".into()),
        message,
    }
}

fn inactive_status(key: &str, message: String) -> LicenseStatus {
    LicenseStatus {
        active: false,
        key_hint: hint(key),
        status: None,
        message: Some(message),
    }
}

/// A stored key is usable by the hosted backend only after ownership was proven for this build.
pub fn stored_key(secrets: &Arc<dyn SecretStore>) -> Option<String> {
    let config = config().ok()?;
    let key = secrets.get(KEY_NAME).ok().flatten()?;
    let instance = secrets.get(INSTANCE_NAME).ok().flatten()?;
    let record = proof(secrets)?;
    proof_valid(
        &record,
        &config,
        &key,
        &instance,
        chrono::Utc::now().timestamp(),
    )
    .then_some(key)
}

pub async fn activate(
    secrets: Arc<dyn SecretStore>,
    key: &str,
    instance_name: &str,
) -> Result<LicenseStatus, String> {
    let key = key.trim();
    if key.is_empty() {
        return Err("The license key is empty.".into());
    }
    let config = config()?;
    let _guard = GATE.lock().await;
    let old_values = [KEY_NAME, INSTANCE_NAME, PROOF_NAME]
        .into_iter()
        .map(|name| {
            secrets.get(name).map_err(|_| {
                "Could not read the existing license from the system credential store.".to_string()
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    // Validation without an instance has no activation side effect. Other products' keys
    // must fail here, before Lemon Squeezy allocates any device activation.
    let checked = call("validate", &[("license_key", key)])
        .await
        .map_err(|e| e.message().to_string())?;
    if !checked.valid || !usable_key(&checked, false) {
        return Err(rejection(&checked));
    }
    ownership(&checked, &config)?;

    if old_values[0].as_deref() == Some(key) {
        if let Some(instance) = old_values[1].as_deref() {
            let checked_instance = call(
                "validate",
                &[("license_key", key), ("instance_id", instance)],
            )
            .await
            .map_err(|e| e.message().to_string())?;
            if checked_instance.valid
                && usable_key(&checked_instance, true)
                && checked_instance
                    .instance
                    .as_ref()
                    .is_some_and(|value| value.id == instance)
            {
                ownership(&checked_instance, &config)?;
                remember(&secrets, &config, key, instance, &checked_instance)?;
                return Ok(active_status(key, None));
            }
        }
    }

    let response = call(
        "activate",
        &[("license_key", key), ("instance_name", instance_name)],
    )
    .await
    .map_err(|e| e.message().to_string())?;
    if !response.activated {
        return Err(rejection(&response));
    }
    let instance = response
        .instance
        .as_ref()
        .filter(|value| !value.id.is_empty())
        .ok_or("The license service did not return an activation instance.")?
        .id
        .clone();
    if ownership(&response, &config).is_err() || !usable_key(&response, true) {
        let _ = call(
            "deactivate",
            &[("license_key", key), ("instance_id", &instance)],
        )
        .await;
        return Err(
            "The activation did not match the configured Pro product and was not saved.".into(),
        );
    }
    // Preserve the previous license if the credential store cannot save the replacement.
    let saved = (|| {
        secrets.set(KEY_NAME, key).map_err(|_| {
            "Could not save the license in the system credential store.".to_string()
        })?;
        secrets.set(INSTANCE_NAME, &instance).map_err(|_| {
            "Could not save the license instance in the system credential store.".to_string()
        })?;
        remember(&secrets, &config, key, &instance, &response)
    })();
    if let Err(error) = saved {
        for (name, old) in [KEY_NAME, INSTANCE_NAME, PROOF_NAME]
            .into_iter()
            .zip(old_values)
        {
            match old {
                Some(value) => {
                    let _ = secrets.set(name, &value);
                }
                None => {
                    let _ = secrets.delete(name);
                }
            }
        }
        let _ = call(
            "deactivate",
            &[("license_key", key), ("instance_id", &instance)],
        )
        .await;
        return Err(error);
    }
    log::info!("Pro license activated and product ownership verified");
    Ok(active_status(key, None))
}

pub async fn validate(secrets: Arc<dyn SecretStore>) -> LicenseStatus {
    // A clean open-source build neither calls Lemon Squeezy nor modifies existing credentials.
    let Ok(config) = config() else {
        return LicenseStatus::default();
    };
    let _guard = GATE.lock().await;
    let (Ok(Some(key)), Ok(Some(instance))) = (secrets.get(KEY_NAME), secrets.get(INSTANCE_NAME))
    else {
        return LicenseStatus::default();
    };
    match call("validate", &[("license_key", &key), ("instance_id", &instance)]).await {
        Ok(response) => {
            let ownership_error = ownership(&response, &config).err();
            if response.valid && ownership_error.is_none() && usable_key(&response, true)
                && response.instance.as_ref().is_some_and(|value| value.id == instance) {
                return match remember(&secrets, &config, &key, &instance, &response) {
                    Ok(()) => active_status(&key, None),
                    Err(error) => inactive_status(&key, error),
                };
            }
            let _ = secrets.delete(PROOF_NAME);
            inactive_status(&key, ownership_error.unwrap_or_else(|| rejection(&response)))
        }
        Err(CallError::Unavailable) if proof(&secrets).is_some_and(|record| proof_valid(&record, &config, &key, &instance, chrono::Utc::now().timestamp())) => {
            active_status(&key, Some("Offline: using a recently verified Pro license. Connect to refresh its status.".into()))
        }
        Err(error) => inactive_status(&key, error.message().into()),
    }
}

/// Deactivation is only reported as successful once Lemon confirms that the device was freed.
pub async fn deactivate(secrets: Arc<dyn SecretStore>) -> Result<(), String> {
    let config = config()?;
    let _guard = GATE.lock().await;
    if let (Ok(Some(key)), Ok(Some(instance))) = (secrets.get(KEY_NAME), secrets.get(INSTANCE_NAME))
    {
        let checked = call(
            "validate",
            &[("license_key", &key), ("instance_id", &instance)],
        )
        .await
        .map_err(|e| e.message().to_string())?;
        ownership(&checked, &config)?;
        let response = call(
            "deactivate",
            &[("license_key", &key), ("instance_id", &instance)],
        )
        .await
        .map_err(|e| e.message().to_string())?;
        if !response.deactivated {
            return Err("The device could not be deactivated. Please try again.".into());
        }
    }
    for name in [PROOF_NAME, INSTANCE_NAME, KEY_NAME] {
        secrets.delete(name).map_err(|_| {
            "Could not remove the license from the system credential store.".to_string()
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn example_config() -> LicenseConfig {
        parse_config(
            Some("12"),
            Some("34"),
            Some("56, 57"),
            "https://example.supabase.co",
            "https://example.supabase.co/functions/v1",
        )
        .unwrap()
    }
    fn response(store: u64, product: u64, variant: u64) -> ApiResponse {
        serde_json::from_value(serde_json::json!({ "valid": true, "license_key": { "status": "active", "expires_at": null }, "instance": { "id": "instance" }, "meta": { "store_id": store, "product_id": product, "variant_id": variant } })).unwrap()
    }
    #[test]
    fn hint_only_for_long_keys() {
        assert_eq!(hint("short"), None);
        assert_eq!(hint("ABCD-EFGH-IJKL-MNOP").as_deref(), Some("…MNOP"));
    }
    #[test]
    fn empty_key_is_rejected_without_network() {
        let store = crate::secrets::MemorySecretStore::default();
        let err =
            tauri::async_runtime::block_on(activate(Arc::new(store), "   ", "test")).unwrap_err();
        assert!(err.contains("empty"), "{err}");
    }
    #[test]
    fn no_license_means_free() {
        let store = crate::secrets::MemorySecretStore::default();
        let status = tauri::async_runtime::block_on(validate(Arc::new(store)));
        assert_eq!(status, LicenseStatus::default());
    }
    #[test]
    fn all_three_product_identifiers_are_required() {
        let config = example_config();
        assert!(ownership(&response(12, 34, 56), &config).is_ok());
        assert!(ownership(&response(12, 34, 57), &config).is_ok());
        for candidate in [
            response(13, 34, 56),
            response(12, 35, 56),
            response(12, 34, 58),
            ApiResponse::default(),
        ] {
            assert!(ownership(&candidate, &config).is_err());
        }
        assert!(parse_config(None, Some("34"), Some("56"), "", "").is_err());
        assert!(parse_config(Some("12"), Some("34"), Some("56,"), "", "").is_err());
        assert!(parse_config(Some("12"), Some("34"), Some("0"), "", "").is_err());
    }
    #[test]
    fn cached_pro_requires_recent_matching_ownership_and_unexpired_key() {
        let config = example_config();
        let mut record = VerifiedLicense {
            key_digest: digest("key"),
            instance_id: "instance".into(),
            config_binding: config.binding(),
            ownership: response(12, 34, 56).meta.unwrap(),
            verified_at: 1_000_000,
            expires_at: None,
        };
        assert!(proof_valid(&record, &config, "key", "instance", 1_000_010));
        assert!(!proof_valid(
            &record,
            &config,
            "other-key",
            "instance",
            1_000_010
        ));
        assert!(!proof_valid(
            &record,
            &config,
            "key",
            "other-instance",
            1_000_010
        ));
        assert!(!proof_valid(
            &record,
            &config,
            "key",
            "instance",
            1_000_000 + MAX_OFFLINE_SECONDS + 1
        ));
        let other_project = parse_config(
            Some("12"),
            Some("34"),
            Some("56,57"),
            "https://other.supabase.co",
            "https://other.supabase.co/functions/v1",
        )
        .unwrap();
        assert!(!proof_valid(
            &record,
            &other_project,
            "key",
            "instance",
            1_000_010
        ));
        record.expires_at = Some("1970-01-01T00:00:00Z".into());
        assert!(!proof_valid(&record, &config, "key", "instance", 1_000_010));
        record.expires_at = Some("malformed".into());
        assert!(!proof_valid(&record, &config, "key", "instance", 1_000_010));
    }
    #[test]
    fn checkout_needs_an_explicit_secure_checkout_path() {
        assert!(parse_checkout_url(Some(
            "https://example.lemonsqueezy.com/buy/example?discount=TEST"
        ))
        .is_ok());
        for value in [
            None,
            Some("https://example.lemonsqueezy.com/"),
            Some("http://example.com/buy/test"),
            Some("https://name:secret@example.com/buy/test"),
            Some("javascript:alert(1)"),
        ] {
            assert!(parse_checkout_url(value).is_err());
        }
    }
}
