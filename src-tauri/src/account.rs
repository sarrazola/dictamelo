//! Supabase email-code authentication. Session credentials stay in the OS credential store.
use crate::secrets::SecretStore;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

pub const SUPABASE_URL: &str = "https://iburiyhhfodndqgmsaot.supabase.co";
const PUBLIC_KEY: &str = include_str!("supabase-public-key.txt");
const SESSION_KEY: &str = "supabase_session";

#[derive(Clone, Serialize, Deserialize)]
struct Session {
    access_token: String,
    refresh_token: String,
    expires_at: i64,
    email: String,
}

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountStatus {
    pub signed_in: bool,
    pub email: Option<String>,
    pub used_words: Option<u64>,
    pub limit_words: u64,
    pub resets_at: Option<String>,
    pub error: Option<String>,
}

pub struct Account {
    secrets: Arc<dyn SecretStore>,
    // Serializes refresh-token rotation and sign-out, across all concurrent commands.
    gate: Mutex<()>,
}

impl Account {
    pub fn new(secrets: Arc<dyn SecretStore>) -> Self { Self { secrets, gate: Mutex::new(()) } }

    fn session(&self) -> Result<Option<Session>, String> {
        self.secrets.get(SESSION_KEY).map_err(|e| e.to_string())?
            .map(|s| serde_json::from_str(&s).map_err(|_| "Please sign in again.".to_string())).transpose()
    }

    pub fn signed_in(&self) -> bool { self.session().ok().flatten().is_some() }

    async fn auth(&self, path: &str, body: Value) -> Result<Value, String> {
        let response = crate::transcription::shared_http_client()
            .post(format!("{SUPABASE_URL}/auth/v1/{path}"))
            .header("apikey", PUBLIC_KEY.trim()).timeout(Duration::from_secs(20))
            .json(&body).send().await.map_err(|_| "Could not connect. Please try again.".to_string())?;
        let status = response.status();
        let data: Value = response.json().await.map_err(|_| "Invalid sign-in response.".to_string())?;
        if !status.is_success() {
            return Err(data["msg"].as_str().or(data["error_description"].as_str())
                .or(data["message"].as_str()).unwrap_or("Sign-in failed. Please try again.").to_string());
        }
        Ok(data)
    }

    pub async fn send_code(&self, email: &str) -> Result<(), String> {
        let email = email.trim();
        if !email.contains('@') || email.len() > 254 { return Err("Enter a valid email address.".into()); }
        self.auth("otp", json!({ "email": email, "create_user": true })).await?;
        Ok(())
    }

    fn save(&self, data: Value) -> Result<Session, String> {
        let session = Session {
            access_token: data["access_token"].as_str().ok_or("Missing session.")?.into(),
            refresh_token: data["refresh_token"].as_str().ok_or("Missing refresh token.")?.into(),
            expires_at: chrono::Utc::now().timestamp() + data["expires_in"].as_i64().unwrap_or(3600),
            email: data["user"]["email"].as_str().ok_or("Missing account email.")?.into(),
        };
        self.secrets.set(SESSION_KEY, &serde_json::to_string(&session).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        Ok(session)
    }

    pub async fn verify_code(&self, email: &str, code: &str) -> Result<(), String> {
        let _guard = self.gate.lock().await;
        let code = code.trim();
        if !(6..=10).contains(&code.len()) || !code.bytes().all(|b| b.is_ascii_digit()) {
            return Err("Enter the verification code from your email.".into());
        }
        let data = self.auth("verify", json!({ "email": email.trim(), "token": code, "type": "email" })).await?;
        self.save(data)?;
        Ok(())
    }

    pub async fn token(&self) -> Result<String, String> {
        let _guard = self.gate.lock().await;
        let mut session = self.session()?.ok_or("Sign in to use your free words.")?;
        if session.expires_at <= chrono::Utc::now().timestamp() + 60 {
            let data = self.auth("token?grant_type=refresh_token", json!({ "refresh_token": session.refresh_token })).await?;
            session = self.save(data)?;
        }
        Ok(session.access_token)
    }

    pub async fn sign_out(&self) -> Result<(), String> {
        let _guard = self.gate.lock().await;
        if let Some(session) = self.session()? {
            // Local sign-out also works offline; revoke this session remotely when reachable.
            let _ = crate::transcription::shared_http_client().post(format!("{SUPABASE_URL}/auth/v1/logout?scope=local"))
                .header("apikey", PUBLIC_KEY.trim()).bearer_auth(session.access_token)
                .timeout(Duration::from_secs(10)).send().await;
        }
        self.secrets.delete(SESSION_KEY).map_err(|e| e.to_string())
    }

    pub async fn status(&self) -> AccountStatus {
        let mut result = AccountStatus { limit_words: 2000, ..Default::default() };
        match self.session() {
            Ok(Some(s)) => { result.signed_in = true; result.email = Some(s.email); }
            Ok(None) => return result,
            Err(e) => { result.error = Some(e); return result; }
        }
        match self.usage().await {
            Ok(data) => {
                result.used_words = data["usedWords"].as_u64();
                result.limit_words = data["limitWords"].as_u64().unwrap_or(2000);
                result.resets_at = data["resetsAt"].as_str().map(str::to_string);
            }
            Err(e) => result.error = Some(e),
        }
        result
    }

    async fn usage(&self) -> Result<Value, String> {
        let token = self.token().await?;
        let response = crate::transcription::shared_http_client().post(format!("{SUPABASE_URL}/functions/v1/usage"))
            .bearer_auth(token).timeout(Duration::from_secs(15)).json(&json!({}))
            .send().await.map_err(|_| "Could not refresh usage. Connect to the internet and try again.".to_string())?;
        let status = response.status();
        let data: Value = response.json().await.map_err(|_| "Could not read usage.".to_string())?;
        if !status.is_success() { return Err(data["error"].as_str().unwrap_or("Could not refresh usage.").into()); }
        Ok(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn session_survives_account_recreation_in_credential_store() {
        let store: Arc<dyn SecretStore> = Arc::new(crate::secrets::MemorySecretStore::default());
        let account = Account::new(store.clone());
        assert!(!account.signed_in());
        account.save(json!({"access_token":"test-access", "refresh_token":"test-refresh", "expires_in":3600, "user":{"email":"test@example.invalid"}})).unwrap();
        let reopened = Account::new(store.clone());
        assert!(reopened.signed_in());
        assert_eq!(reopened.session().unwrap().unwrap().email, "test@example.invalid");
        store.delete(SESSION_KEY).unwrap();
        assert!(!reopened.signed_in());
    }
    #[test]
    fn public_status_never_contains_session_credentials() {
        let value = serde_json::to_value(AccountStatus::default()).unwrap();
        for field in ["access_token", "refresh_token", "accessToken", "refreshToken"] {
            assert!(value.get(field).is_none());
        }
        assert!(value["usedWords"].is_null(), "Unavailable usage must not be shown as zero");
    }
}
