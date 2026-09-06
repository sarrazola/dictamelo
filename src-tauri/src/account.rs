//! Supabase account authentication. Passwords are never persisted and session credentials
//! stay in the OS credential store. Google signs in in the browser with PKCE; the Google
//! client secret belongs to Supabase's server configuration, never this application.
use crate::secrets::SecretStore;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{watch, Mutex};
use uuid::Uuid;

use crate::cloud_config::{anon_key, backend_url, supabase_url};
const SESSION_KEY: &str = "supabase_session";
const GOOGLE_TIMEOUT: Duration = Duration::from_secs(300);
const MAX_CALLBACK_HEADER: usize = 8192;

#[derive(Clone, Serialize, Deserialize)]
struct Session {
    access_token: String,
    refresh_token: String,
    expires_at: i64,
    email: String,
    #[serde(default)]
    supabase_url: String,
}

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountStatus {
    pub signed_in: bool,
    pub email: Option<String>,
    pub used_words: Option<u64>,
    pub limit_words: u64,
    pub used_seconds: Option<f64>,
    pub limit_seconds: f64,
    pub resets_at: Option<String>,
    pub error: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SignUpResult {
    pub status: AccountStatus,
    pub confirmation_required: bool,
}

pub struct Account {
    secrets: Arc<dyn SecretStore>,
    // Serializes refresh-token rotation and every mutation of the saved session.
    gate: Mutex<()>,
    google_flow: StdMutex<Option<(Uuid, watch::Sender<bool>)>>,
}

impl Account {
    pub fn new(secrets: Arc<dyn SecretStore>) -> Self {
        Self {
            secrets,
            gate: Mutex::new(()),
            google_flow: StdMutex::new(None),
        }
    }

    fn session(&self) -> Result<Option<Session>, String> {
        self.secrets
            .get(SESSION_KEY)
            .map_err(|_| {
                "Could not read your account from the system credential store.".to_string()
            })?
            .map(|s| serde_json::from_str(&s).map_err(|_| "Please sign in again.".to_string()))
            .transpose()
    }

    pub fn signed_in(&self) -> bool {
        supabase_url()
            .ok()
            .and_then(|url| self.project_session(url).ok())
            .flatten()
            .is_some()
    }

    fn project_session(&self, base_url: &str) -> Result<Option<Session>, String> {
        match self.session()? {
            Some(session) if session.supabase_url == base_url => Ok(Some(session)),
            Some(_) => Err("Please sign in again to connect your account to this version.".into()),
            None => Ok(None),
        }
    }

    async fn auth(&self, path: &str, body: Value) -> Result<Value, String> {
        let response = crate::transcription::shared_http_client()
            .post(format!("{}/auth/v1/{path}", supabase_url()?))
            .header("apikey", anon_key()?)
            .timeout(Duration::from_secs(20))
            .json(&body)
            .send()
            .await
            .map_err(|_| "Could not connect. Please try again.".to_string())?;
        auth_response(response).await
    }

    pub async fn sign_up(&self, email: &str, password: &str) -> Result<bool, String> {
        validate_email(email)?;
        validate_password(password)?;
        self.cancel_google();
        let _guard = self.gate.lock().await;
        let data = self
            .auth(
                "signup",
                json!({ "email": email.trim(), "password": password }),
            )
            .await?;
        if data["access_token"].is_string() {
            self.save(data)?;
            Ok(false)
        } else {
            // Supabase deliberately uses the same response for some existing accounts.
            // Do not claim that a new account definitely exists until it is confirmed.
            Ok(true)
        }
    }

    pub async fn sign_in(&self, email: &str, password: &str) -> Result<(), String> {
        validate_email(email)?;
        if password.is_empty() {
            return Err("Enter your password.".into());
        }
        self.cancel_google();
        let _guard = self.gate.lock().await;
        let data = self
            .auth(
                "token?grant_type=password",
                json!({ "email": email.trim(), "password": password }),
            )
            .await?;
        self.save(data)?;
        Ok(())
    }

    pub async fn confirm_email(&self, email: &str, code: &str) -> Result<(), String> {
        validate_email(email)?;
        validate_code(code)?;
        self.cancel_google();
        let _guard = self.gate.lock().await;
        let data = self
            .auth(
                "verify",
                json!({ "email": email.trim(), "token": code.trim(), "type": "signup" }),
            )
            .await?;
        self.save(data)?;
        Ok(())
    }

    pub async fn request_password_reset(&self, email: &str) -> Result<(), String> {
        validate_email(email)?;
        self.auth("recover", json!({ "email": email.trim() }))
            .await?;
        Ok(())
    }

    pub async fn resend_confirmation(&self, email: &str) -> Result<(), String> {
        validate_email(email)?;
        self.auth("resend", json!({ "email": email.trim(), "type": "signup" }))
            .await?;
        Ok(())
    }

    pub async fn reset_password(
        &self,
        email: &str,
        code: &str,
        password: &str,
    ) -> Result<(), String> {
        validate_email(email)?;
        validate_code(code)?;
        validate_password(password)?;
        self.cancel_google();
        let _guard = self.gate.lock().await;
        let data = self
            .auth(
                "verify",
                json!({ "email": email.trim(), "token": code.trim(), "type": "recovery" }),
            )
            .await?;
        let access_token = data["access_token"]
            .as_str()
            .ok_or("Missing recovery session. Request another recovery email.")?;
        let response = crate::transcription::shared_http_client()
            .put(format!("{}/auth/v1/user", supabase_url()?))
            .header("apikey", anon_key()?)
            .bearer_auth(access_token)
            .timeout(Duration::from_secs(20))
            .json(&json!({ "password": password }))
            .send()
            .await
            .map_err(|_| "Could not change your password. Please try again.".to_string())?;
        auth_response(response).await?;
        // A recovery session is saved only after the password was actually changed.
        self.save(data)?;
        Ok(())
    }

    // Retained for existing integrations; normal UI sign-in now uses a password or Google.
    pub async fn send_code(&self, email: &str) -> Result<(), String> {
        validate_email(email)?;
        self.auth("otp", json!({ "email": email.trim(), "create_user": true }))
            .await?;
        Ok(())
    }

    fn save(&self, data: Value) -> Result<Session, String> {
        self.save_for_project(data, supabase_url()?)
    }

    fn save_for_project(&self, data: Value, base_url: &str) -> Result<Session, String> {
        let session = Session {
            supabase_url: base_url.into(),
            access_token: data["access_token"]
                .as_str()
                .ok_or("Missing session.")?
                .into(),
            refresh_token: data["refresh_token"]
                .as_str()
                .ok_or("Missing refresh token.")?
                .into(),
            expires_at: chrono::Utc::now().timestamp()
                + data["expires_in"].as_i64().unwrap_or(3600),
            email: data["user"]["email"]
                .as_str()
                .ok_or("Missing account email.")?
                .into(),
        };
        self.secrets
            .set(
                SESSION_KEY,
                &serde_json::to_string(&session)
                    .map_err(|_| "Could not save your account.".to_string())?,
            )
            .map_err(|_| {
                "Could not save your account in the system credential store.".to_string()
            })?;
        Ok(session)
    }

    pub async fn verify_code(&self, email: &str, code: &str) -> Result<(), String> {
        validate_email(email)?;
        validate_code(code)?;
        self.cancel_google();
        let _guard = self.gate.lock().await;
        let data = self
            .auth(
                "verify",
                json!({ "email": email.trim(), "token": code.trim(), "type": "email" }),
            )
            .await?;
        self.save(data)?;
        Ok(())
    }

    pub fn cancel_google(&self) {
        if let Some((_, cancel)) = self
            .google_flow
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
        {
            let _ = cancel.send(true);
        }
    }

    pub async fn sign_in_google(&self) -> Result<(), String> {
        let id = Uuid::new_v4();
        let (cancel, receiver) = watch::channel(false);
        {
            let mut pending = self.google_flow.lock().unwrap_or_else(|e| e.into_inner());
            if pending.is_some() {
                return Err("Google sign-in is already open. Finish or cancel it first.".into());
            }
            *pending = Some((id, cancel));
        }
        let result = self.google_browser_flow(receiver).await;
        let mut pending = self.google_flow.lock().unwrap_or_else(|e| e.into_inner());
        if pending
            .as_ref()
            .is_some_and(|(active_id, _)| *active_id == id)
        {
            *pending = None;
        }
        result
    }

    async fn google_browser_flow(
        &self,
        mut cancelled: watch::Receiver<bool>,
    ) -> Result<(), String> {
        // Bind first so another process cannot take over this callback port. The per-flow
        // random path is an additional state nonce; Supabase also validates OAuth state.
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .map_err(|_| {
                "Could not open the local sign-in callback. Please try again.".to_string()
            })?;
        let address = listener
            .local_addr()
            .map_err(|_| "Could not open the sign-in callback.".to_string())?;
        let path = format!("/auth/callback/{}", Uuid::new_v4().simple());
        let host = address.to_string();
        let redirect = format!("http://{address}{path}");
        let verifier = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        let authorize = google_authorize_url(supabase_url()?, &redirect, &verifier)?;
        if *cancelled.borrow() {
            return Err("Google sign-in was cancelled.".into());
        }
        tauri_plugin_opener::open_url(authorize, None::<&str>)
            .map_err(|_| "Could not open your browser. Please try again.".to_string())?;
        let (mut stream, code) = tokio::select! {
            _ = cancelled.changed() => return Err("Google sign-in was cancelled.".into()),
            result = tokio::time::timeout(GOOGLE_TIMEOUT, accept_callback(&listener, &host, &path)) => {
                result.map_err(|_| "Google sign-in timed out. Please try again.".to_string())??
            }
        };
        let result = async {
            let _guard = self.gate.lock().await;
            if *cancelled.borrow() {
                return Err("Google sign-in was cancelled.".to_string());
            }
            let data = self
                .auth(
                    "token?grant_type=pkce",
                    json!({ "auth_code": code, "code_verifier": verifier }),
                )
                .await?;
            if *cancelled.borrow() {
                return Err("Google sign-in was cancelled.".to_string());
            }
            self.save(data)?;
            Ok(())
        }
        .await;
        let message = if result.is_ok() {
            "You are signed in. You can close this tab and return to Dictámelo."
        } else {
            "Sign-in could not be completed. Return to Dictámelo and try again."
        };
        browser_response(&mut stream, 200, message).await;
        result
    }

    pub async fn token(&self) -> Result<String, String> {
        let _guard = self.gate.lock().await;
        let mut session = self
            .project_session(supabase_url()?)?
            .ok_or("Sign in to use your free audio allowance.")?;
        if session.expires_at <= chrono::Utc::now().timestamp() + 60 {
            let data = self
                .auth(
                    "token?grant_type=refresh_token",
                    json!({ "refresh_token": session.refresh_token }),
                )
                .await?;
            session = self.save(data)?;
        }
        Ok(session.access_token)
    }

    pub async fn sign_out(&self) -> Result<(), String> {
        self.cancel_google();
        let _guard = self.gate.lock().await;
        let session = self.session().ok().flatten();
        // Delete first so corrupt credentials and offline revocation cannot prevent logout.
        self.secrets.delete(SESSION_KEY).map_err(|_| {
            "Could not remove the account from the system credential store.".to_string()
        })?;
        if let (Some(session), Ok(base_url), Ok(public_key)) = (session, supabase_url(), anon_key())
        {
            if session.supabase_url != base_url {
                return Ok(());
            }
            let _ = crate::transcription::shared_http_client()
                .post(format!("{base_url}/auth/v1/logout?scope=local"))
                .header("apikey", public_key)
                .bearer_auth(session.access_token)
                .timeout(Duration::from_secs(10))
                .send()
                .await;
        }
        Ok(())
    }

    pub async fn status(&self) -> AccountStatus {
        let mut result = AccountStatus {
            limit_words: 2000,
            limit_seconds: 1800.0,
            ..Default::default()
        };
        match supabase_url().and_then(|url| self.project_session(url)) {
            Ok(Some(s)) => {
                result.signed_in = true;
                result.email = Some(s.email);
            }
            Ok(None) => return result,
            Err(e) => {
                result.error = Some(e);
                return result;
            }
        }
        match self.usage().await {
            Ok(data) => {
                result.used_seconds = data["usedSeconds"].as_f64();
                result.limit_seconds = data["limitSeconds"].as_f64().unwrap_or(1800.0);
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
        let response = crate::transcription::shared_http_client()
            .post(format!("{}/usage", backend_url()?))
            .bearer_auth(token)
            .timeout(Duration::from_secs(15))
            .json(&json!({}))
            .send()
            .await
            .map_err(|_| {
                "Could not refresh usage. Connect to the internet and try again.".to_string()
            })?;
        let status = response.status();
        let data: Value = response
            .json()
            .await
            .map_err(|_| "Could not read usage.".to_string())?;
        if !status.is_success() {
            return Err("Could not refresh usage. Please try again.".into());
        }
        Ok(data)
    }
}

fn validate_email(email: &str) -> Result<(), String> {
    let email = email.trim();
    let Some((local, domain)) = email.rsplit_once('@') else {
        return Err("Enter a valid email address.".into());
    };
    if local.is_empty()
        || domain.is_empty()
        || local.contains('@')
        || email.chars().any(char::is_whitespace)
        || email.len() > 254
    {
        return Err("Enter a valid email address.".into());
    }
    Ok(())
}

fn validate_password(password: &str) -> Result<(), String> {
    if password.chars().count() < 8 {
        return Err("Use a password with at least 8 characters.".into());
    }
    if password.len() > 72 {
        return Err("Use a password of at most 72 bytes.".into());
    }
    Ok(())
}

fn validate_code(code: &str) -> Result<(), String> {
    let code = code.trim();
    if !(6..=10).contains(&code.len()) || !code.bytes().all(|b| b.is_ascii_digit()) {
        return Err("Enter the verification code from your email.".into());
    }
    Ok(())
}

async fn auth_response(response: reqwest::Response) -> Result<Value, String> {
    let status = response.status();
    let data: Value = response
        .json()
        .await
        .map_err(|_| "Invalid account response. Please try again.".to_string())?;
    if !status.is_success() {
        return Err(auth_error(&data, status.as_u16()));
    }
    Ok(data)
}

// Do not forward arbitrary provider responses, callback query strings, or tokens to UI/logs.
fn auth_error(data: &Value, status: u16) -> String {
    match data["error_code"]
        .as_str()
        .or(data["code"].as_str())
        .unwrap_or("")
    {
        "invalid_credentials" => "Email or password is incorrect.",
        "email_not_confirmed" => "Confirm your email before signing in.",
        "weak_password" | "validation_failed" => {
            "Check your email and use a password with at least 8 characters."
        }
        "same_password" => "Choose a password different from your current password.",
        "over_email_send_rate_limit" | "over_request_rate_limit" => {
            "Too many attempts. Please wait a minute and try again."
        }
        "email_address_not_authorized" => {
            "Email delivery is not ready yet. Try Google sign-in or contact support."
        }
        "otp_expired" | "otp_disabled" => {
            "This verification code is invalid or expired. Request a new email."
        }
        "user_already_exists" | "email_exists" => {
            "An account with this email already exists. Sign in instead."
        }
        "provider_disabled" => "Google sign-in is not available yet. Use email and password.",
        "flow_state_expired" | "flow_state_not_found" | "bad_code_verifier" => {
            "Google sign-in expired. Please start again."
        }
        "refresh_token_not_found"
        | "refresh_token_already_used"
        | "session_not_found"
        | "session_expired" => "Your session expired. Please sign in again.",
        _ if status == 429 => "Too many attempts. Please wait a minute and try again.",
        _ => "The account request could not be completed. Please try again.",
    }
    .into()
}

fn google_authorize_url(base_url: &str, redirect: &str, verifier: &str) -> Result<String, String> {
    let mut url = reqwest::Url::parse(&format!("{base_url}/auth/v1/authorize"))
        .map_err(|_| "The sign-in service is not configured correctly.".to_string())?;
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    url.query_pairs_mut().extend_pairs([
        ("provider", "google"),
        ("redirect_to", redirect),
        ("code_challenge", &challenge),
        ("code_challenge_method", "s256"),
    ]);
    Ok(url.into())
}

fn callback_code(request: &str, host: &str, path: &str) -> Result<Option<String>, ()> {
    let mut lines = request.split("\r\n");
    let mut request_line = lines.next().ok_or(())?.split_whitespace();
    if request_line.next() != Some("GET") {
        return Err(());
    }
    let target = request_line.next().ok_or(())?;
    if request_line.next() != Some("HTTP/1.1")
        || request_line.next().is_some()
        || !target.starts_with('/')
        || target.starts_with("//")
    {
        return Err(());
    }
    let hosts: Vec<_> = lines
        .filter_map(|line| line.split_once(':'))
        .filter(|(name, _)| name.eq_ignore_ascii_case("host"))
        .map(|(_, value)| value.trim())
        .collect();
    if hosts != [host] {
        return Err(());
    }
    let url = reqwest::Url::parse(&format!("http://{host}{target}")).map_err(|_| ())?;
    if url.path() != path {
        return Err(());
    }
    let pairs: Vec<_> = url.query_pairs().collect();
    if pairs.iter().any(|(key, _)| key == "error") {
        return Ok(None);
    }
    let codes: Vec<_> = pairs.iter().filter(|(key, _)| key == "code").collect();
    if codes.len() != 1 || codes[0].1.is_empty() || codes[0].1.len() > 2048 {
        return Err(());
    }
    Ok(Some(codes[0].1.to_string()))
}

async fn accept_callback(
    listener: &TcpListener,
    host: &str,
    path: &str,
) -> Result<(TcpStream, String), String> {
    loop {
        let (mut stream, peer) = listener
            .accept()
            .await
            .map_err(|_| "Could not receive the browser sign-in response.".to_string())?;
        if !peer.ip().is_loopback() {
            continue;
        }
        let request = tokio::time::timeout(Duration::from_secs(3), async {
            let mut bytes = Vec::new();
            let mut chunk = [0; 1024];
            while bytes.len() < MAX_CALLBACK_HEADER {
                let n = stream.read(&mut chunk).await.map_err(|_| ())?;
                if n == 0 {
                    return Err(());
                }
                bytes.extend_from_slice(&chunk[..n]);
                if bytes.len() > MAX_CALLBACK_HEADER {
                    return Err(());
                }
                if bytes.windows(4).any(|w| w == b"\r\n\r\n") {
                    return String::from_utf8(bytes).map_err(|_| ());
                }
            }
            Err(())
        })
        .await;
        match request
            .ok()
            .and_then(Result::ok)
            .and_then(|r| callback_code(&r, host, path).ok())
        {
            Some(Some(code)) => return Ok((stream, code)),
            Some(None) => {
                browser_response(
                    &mut stream,
                    200,
                    "Sign-in was cancelled. Return to Dictámelo to try again.",
                )
                .await;
                return Err("Google sign-in was cancelled or denied. Please try again.".into());
            }
            None => {
                browser_response(&mut stream, 400, "This is not an active sign-in callback.").await
            }
        }
    }
}

async fn browser_response(stream: &mut TcpStream, status: u16, message: &str) {
    let body = format!("<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width\"><title>Dictámelo</title><body><h1>Dictámelo</h1><p>{message}</p></body></html>");
    let reason = if status == 200 { "OK" } else { "Bad Request" };
    let response = format!("HTTP/1.1 {status} {reason}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nContent-Security-Policy: default-src 'none'; frame-ancestors 'none'\r\nReferrer-Policy: no-referrer\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n{body}", body.len());
    let _ = tokio::time::timeout(
        Duration::from_secs(2),
        stream.write_all(response.as_bytes()),
    )
    .await;
    let _ = stream.shutdown().await;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn session_survives_account_recreation_in_credential_store() {
        let store: Arc<dyn SecretStore> = Arc::new(crate::secrets::MemorySecretStore::default());
        let account = Account::new(store.clone());
        assert!(!account.signed_in());
        account.save_for_project(json!({"access_token":"test-access", "refresh_token":"test-refresh", "expires_in":3600, "user":{"email":"test@example.invalid"}}), "https://project.supabase.co").unwrap();
        let reopened = Account::new(store.clone());
        assert!(reopened
            .project_session("https://project.supabase.co")
            .unwrap()
            .is_some());
        assert!(reopened
            .project_session("https://different.supabase.co")
            .is_err());
        assert_eq!(
            reopened.session().unwrap().unwrap().email,
            "test@example.invalid"
        );
        store.delete(SESSION_KEY).unwrap();
        assert!(!reopened.signed_in());
    }
    #[test]
    fn public_status_never_contains_session_credentials() {
        let value = serde_json::to_value(AccountStatus::default()).unwrap();
        for field in [
            "access_token",
            "refresh_token",
            "accessToken",
            "refreshToken",
        ] {
            assert!(value.get(field).is_none());
        }
        assert!(
            value["usedWords"].is_null() && value["usedSeconds"].is_null(),
            "Unavailable usage must not be shown as zero"
        );
        assert_eq!(
            auth_error(&json!({"msg": "secret-token"}), 400),
            "The account request could not be completed. Please try again."
        );
    }
    #[test]
    fn google_pkce_matches_rfc7636_vector_without_sending_verifier() {
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let url = reqwest::Url::parse(
            &google_authorize_url(
                "https://example.supabase.co",
                "http://127.0.0.1:32145/auth/callback/nonce",
                verifier,
            )
            .unwrap(),
        )
        .unwrap();
        let query: std::collections::HashMap<_, _> = url.query_pairs().collect();
        assert_eq!(
            query["code_challenge"],
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
        assert_eq!(query["code_challenge_method"], "s256");
        assert!(!url.as_str().contains(verifier));
        assert!(!query.contains_key("client_secret"));
    }
    #[test]
    fn callback_requires_local_host_nonce_path_and_single_code() {
        let host = "127.0.0.1:32145";
        let path = "/auth/callback/unguessable";
        let request = |target: &str, authority: &str| {
            format!("GET {target} HTTP/1.1\r\nHost: {authority}\r\n\r\n")
        };
        assert_eq!(
            callback_code(&request(&format!("{path}?code=abc"), host), host, path),
            Ok(Some("abc".into()))
        );
        assert!(callback_code(
            &request(&format!("{path}?code=abc"), "attacker.invalid"),
            host,
            path
        )
        .is_err());
        assert!(
            callback_code(&request("/auth/callback/other?code=abc", host), host, path).is_err()
        );
        assert!(callback_code(
            &request(&format!("{path}?code=abc&code=def"), host),
            host,
            path
        )
        .is_err());
        assert!(callback_code(
            &request("http://attacker.invalid/?code=abc", host),
            host,
            path
        )
        .is_err());
        assert_eq!(
            callback_code(
                &request(&format!("{path}?error=access_denied"), host),
                host,
                path
            ),
            Ok(None)
        );
    }
    #[tokio::test]
    async fn loopback_ignores_wrong_path_then_accepts_real_callback() {
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .unwrap();
        let host = listener.local_addr().unwrap().to_string();
        let client_host = host.clone();
        let client = tokio::spawn(async move {
            let mut unrelated = TcpStream::connect(&client_host).await.unwrap();
            unrelated
                .write_all(
                    format!("GET /favicon.ico HTTP/1.1\r\nHost: {client_host}\r\n\r\n").as_bytes(),
                )
                .await
                .unwrap();
            let mut reply = String::new();
            unrelated.read_to_string(&mut reply).await.unwrap();
            assert!(reply.starts_with("HTTP/1.1 400"));
            let mut callback = TcpStream::connect(&client_host).await.unwrap();
            callback.write_all(format!("GET /auth/callback/nonce?code=test-code HTTP/1.1\r\nHost: {client_host}\r\n\r\n").as_bytes()).await.unwrap();
        });
        let (_, code) = tokio::time::timeout(
            Duration::from_secs(5),
            accept_callback(&listener, &host, "/auth/callback/nonce"),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(code, "test-code");
        client.await.unwrap();
    }
    #[tokio::test]
    async fn sign_out_removes_corrupt_session_without_network() {
        let store: Arc<dyn SecretStore> = Arc::new(crate::secrets::MemorySecretStore::default());
        store.set(SESSION_KEY, "not-json").unwrap();
        let account = Account::new(store.clone());
        account.sign_out().await.unwrap();
        assert!(store.get(SESSION_KEY).unwrap().is_none());
    }
}
