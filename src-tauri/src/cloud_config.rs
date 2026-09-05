//! Optional public build configuration for the hosted edition. A clean open-source build
//! works with personal API keys and does not connect to Dictámelo's hosted services.
//! These values identify public endpoints; server/provider secrets must never be supplied.
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use std::sync::OnceLock;

const UNCONFIGURED: &str = "Cloud services are not configured in this build. Use your own API key or install the official Dictámelo app.";

struct CloudConfig {
    supabase_url: String,
    anon_key: String,
    backend_url: String,
}

static CONFIG: OnceLock<Result<CloudConfig, String>> = OnceLock::new();

fn config() -> Result<&'static CloudConfig, String> {
    CONFIG
        .get_or_init(|| {
            parse_config(
                option_env!("DICTAMELO_SUPABASE_URL"),
                option_env!("DICTAMELO_SUPABASE_ANON_KEY"),
                option_env!("DICTAMELO_BACKEND_URL"),
            )
        })
        .as_ref()
        .map_err(Clone::clone)
}

pub fn configured() -> bool {
    config().is_ok()
}
pub fn supabase_url() -> Result<&'static str, String> {
    config().map(|c| c.supabase_url.as_str())
}
pub fn anon_key() -> Result<&'static str, String> {
    config().map(|c| c.anon_key.as_str())
}
pub fn backend_url() -> Result<&'static str, String> {
    config().map(|c| c.backend_url.as_str())
}

fn public_endpoint(value: &str, allow_path: bool) -> Result<String, String> {
    let url = reqwest::Url::parse(value).map_err(|_| "Invalid public cloud URL.".to_string())?;
    let loopback = url.host_str().is_some_and(|host| {
        host == "localhost"
            || host
                .parse::<std::net::IpAddr>()
                .is_ok_and(|ip| ip.is_loopback())
    });
    if !(url.scheme() == "https" || (url.scheme() == "http" && loopback))
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || (!allow_path && url.path() != "/")
    {
        return Err("Cloud URLs must use HTTPS without credentials, query strings, or fragments. HTTP is allowed only for local development.".into());
    }
    Ok(url.as_str().trim_end_matches('/').to_string())
}

fn public_anon_key(value: &str) -> bool {
    if value.is_empty() || value.chars().any(char::is_whitespace) {
        return false;
    }
    if value.starts_with("sb_publishable_") {
        return value.len() > 20;
    }
    let pieces: Vec<_> = value.split('.').collect();
    if pieces.len() != 3 || pieces.iter().any(|piece| piece.is_empty()) {
        return false;
    }
    URL_SAFE_NO_PAD
        .decode(pieces[1])
        .ok()
        .and_then(|payload| serde_json::from_slice::<serde_json::Value>(&payload).ok())
        .is_some_and(|payload| payload["role"] == "anon")
}

fn parse_config(
    url: Option<&str>,
    key: Option<&str>,
    backend: Option<&str>,
) -> Result<CloudConfig, String> {
    let (Some(url), Some(key)) = (
        url.filter(|v| !v.trim().is_empty()),
        key.filter(|v| !v.trim().is_empty()),
    ) else {
        return Err(UNCONFIGURED.into());
    };
    let supabase_url = public_endpoint(url.trim(), false)?;
    let anon_key = key.trim().to_string();
    if !public_anon_key(&anon_key) {
        return Err("The desktop app requires a Supabase public anon or publishable key. Never supply a service-role or secret key.".into());
    }
    let backend_url = match backend.filter(|v| !v.trim().is_empty()) {
        Some(value) => public_endpoint(value.trim(), true)?,
        None => format!("{supabase_url}/functions/v1"),
    };
    Ok(CloudConfig {
        supabase_url,
        anon_key,
        backend_url,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn jwt(role: &str) -> String {
        format!(
            "e30.{}.signature",
            URL_SAFE_NO_PAD.encode(format!("{{\"role\":\"{role}\"}}"))
        )
    }
    #[test]
    fn hosted_services_require_explicit_public_configuration() {
        assert!(parse_config(None, None, None).is_err());
        assert!(parse_config(Some("https://project.supabase.co"), None, None).is_err());
        let config = parse_config(
            Some("https://project.supabase.co/"),
            Some(&jwt("anon")),
            None,
        )
        .unwrap();
        assert_eq!(config.supabase_url, "https://project.supabase.co");
        assert_eq!(
            config.backend_url,
            "https://project.supabase.co/functions/v1"
        );
        assert!(parse_config(
            Some("http://127.0.0.1:54321"),
            Some("sb_publishable_example_key"),
            None
        )
        .is_ok());
    }
    #[test]
    fn cloud_configuration_rejects_privileged_keys_and_unsafe_urls() {
        for secret in [
            jwt("service_role"),
            "sb_secret_example_key".into(),
            "provider-private-key".into(),
        ] {
            assert!(
                parse_config(Some("https://project.supabase.co"), Some(&secret), None).is_err()
            );
        }
        for url in [
            "http://example.com",
            "https://user:password@example.com",
            "https://example.com?secret=value",
            "https://example.com#token",
            "file:///tmp/config",
            "https://example.com/auth",
        ] {
            assert!(parse_config(Some(url), Some(&jwt("anon")), None).is_err());
        }
        assert!(parse_config(
            Some("https://project.supabase.co"),
            Some(&jwt("anon")),
            Some("http://insecure.example.com/api")
        )
        .is_err());
    }
}
