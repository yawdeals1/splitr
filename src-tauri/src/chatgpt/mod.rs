//! Connecting a ChatGPT account instead of an API key (#31), so Smart mode uses the user's ChatGPT
//! plan. It follows OpenAI's Sign in with ChatGPT guide for open-source and locally hosted apps.
//!
//! The tokens live only in the OS credential store (NFR-05), never in a file, a log or the
//! interface. The interface learns only whether ChatGPT is connected, and the account's name, email
//! and plan. What isn't secret (this installation's host id and the client id OpenAI issued) is
//! kept in `chatgpt.json` next to the settings, so later sign-ins reuse them.

pub mod jwt;
pub mod oauth;

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;

use crate::error::{Result, SplitError};
use crate::keystore::SecretStore;

pub const FILE_NAME: &str = "chatgpt.json";

/// OpenAI's sign-in addresses. Tests point them at a local server.
pub struct Endpoints {
    pub authorize: String,
    pub token: String,
    pub revoke: String,
    pub discovery: String,
    pub issuer: String,
}

impl Default for Endpoints {
    fn default() -> Self {
        Self {
            authorize: "https://auth.openai.com/api/accounts/authorize".into(),
            token: "https://auth.openai.com/api/accounts/oauth/token".into(),
            revoke: "https://auth.openai.com/api/accounts/oauth/revoke".into(),
            discovery: "https://auth.openai.com/.well-known/openid-configuration".into(),
            issuer: "https://auth.openai.com".into(),
        }
    }
}

/// This installation's registration with OpenAI. Not secret: the host id is an opaque random id
/// and the client id is public.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Registration {
    pub host_id: String,
    pub client_id: Option<String>,
}

impl Registration {
    /// The saved registration, or a new host id if there is none yet (or the file is unreadable).
    pub fn load(path: &Path) -> Result<Self> {
        let saved = std::fs::read_to_string(path).ok().and_then(|s| serde_json::from_str::<Self>(&s).ok());
        match saved {
            Some(r) if !r.host_id.is_empty() => Ok(r),
            _ => Ok(Self { host_id: oauth::new_host_id()?, client_id: None }),
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let fail = |e: std::io::Error| SplitError::Ai(format!("Splitr couldn't save its ChatGPT registration ({e})"));
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(fail)?;
        }
        // Serializing two strings can't fail.
        std::fs::write(path, serde_json::to_string_pretty(self).unwrap_or_default()).map_err(fail)
    }
}

/// The account a session belongs to, from its verified ID token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    pub sub: String,
    pub email: Option<String>,
    pub name: Option<String>,
    pub plan: Option<String>,
}

/// A connected account's tokens. Deliberately not `Debug`, so they can't end up in a log.
#[derive(Clone, Serialize, Deserialize)]
pub struct Session {
    pub client_id: String,
    pub access_token: String,
    pub refresh_token: Option<String>,
    /// When the access token expires, in Unix seconds.
    pub expires_at: u64,
    pub scopes: Vec<String>,
    pub account: Account,
}

impl Session {
    fn new(client_id: &str, tokens: oauth::Tokens, account: Account, previous: Option<&Session>) -> Self {
        let scopes = match tokens.scope {
            Some(s) => s.split_whitespace().map(String::from).collect(),
            // A refresh that names no scopes keeps the ones granted before.
            None => previous.map(|p| p.scopes.clone()).unwrap_or_default(),
        };
        Self {
            client_id: client_id.into(),
            access_token: tokens.access_token,
            refresh_token: tokens.refresh_token.or_else(|| previous.and_then(|p| p.refresh_token.clone())),
            expires_at: now() + tokens.expires_in.unwrap_or(3600),
            scopes,
            account,
        }
    }

    /// True when the user allowed Splitr to use their ChatGPT plan.
    pub fn plan_usage(&self) -> bool {
        self.scopes.iter().any(|s| s == oauth::PLAN_SCOPE)
    }
}

pub fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// What the interface may know about the connection (NFR-05): never a token.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub connected: bool,
    pub email: Option<String>,
    pub name: Option<String>,
    pub plan: Option<String>,
    /// False when the user signed in but didn't allow plan usage.
    pub plan_usage: bool,
}

impl Status {
    pub fn of(session: Option<&Session>) -> Self {
        match session {
            None => Self::default(),
            Some(s) => Self {
                connected: true,
                email: s.account.email.clone(),
                name: s.account.name.clone(),
                plan: s.account.plan.clone(),
                plan_usage: s.plan_usage(),
            },
        }
    }
}

// The session is stored in parts: Windows limits one credential to 1,280 characters, and the
// tokens together are longer. `ENTRY` holds the number of parts.
const ENTRY: &str = "chatgpt-session";
const PART_CHARS: usize = 1000;

fn part(i: usize) -> String {
    format!("{ENTRY}-{i}")
}

fn part_count(store: &dyn SecretStore) -> Result<usize> {
    Ok(store.get(ENTRY)?.and_then(|n| n.parse().ok()).unwrap_or(0))
}

pub fn save_session(store: &dyn SecretStore, session: &Session) -> Result<()> {
    delete_session(store)?;
    // Serializing strings and numbers can't fail.
    let json = serde_json::to_string(session).unwrap_or_default();
    let chars: Vec<char> = json.chars().collect();
    let parts: Vec<String> = chars.chunks(PART_CHARS).map(|c| c.iter().collect()).collect();
    for (i, text) in parts.iter().enumerate() {
        store.set(&part(i), text)?;
    }
    store.set(ENTRY, &parts.len().to_string())
}

/// The saved session, if any. A damaged one counts as signed out.
pub fn load_session(store: &dyn SecretStore) -> Result<Option<Session>> {
    let count = part_count(store)?;
    if count == 0 {
        return Ok(None);
    }
    let mut json = String::new();
    for i in 0..count {
        match store.get(&part(i))? {
            Some(text) => json.push_str(&text),
            None => return Ok(None),
        }
    }
    Ok(serde_json::from_str(&json).ok())
}

pub fn delete_session(store: &dyn SecretStore) -> Result<()> {
    let count = part_count(store)?;
    store.delete(ENTRY)?;
    for i in 0..count {
        store.delete(&part(i))?;
    }
    Ok(())
}

/// Signs in: opens the sign-in page with `open_browser`, waits for the callback, exchanges the
/// code, and verifies the ID token. Updates `registration` with the client id OpenAI issued.
pub async fn connect(
    endpoints: &Endpoints,
    registration: &mut Registration,
    open_browser: impl FnOnce(&str) -> Result<()>,
) -> Result<Session> {
    // The listener starts before the browser opens. Only the port may differ between sign-ins.
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| SplitError::Ai(format!("Splitr couldn't start the ChatGPT sign-in ({e})")))?;
    let port = listener.local_addr().map_err(|e| SplitError::Ai(format!("Splitr couldn't start the ChatGPT sign-in ({e})")))?.port();
    let redirect_uri = format!("http://127.0.0.1:{port}{}", oauth::CALLBACK_PATH);
    let pkce = oauth::pkce()?;
    let state = oauth::random_token(24)?;
    let nonce = oauth::random_token(24)?;
    let url = oauth::authorize_url(
        &endpoints.authorize,
        &oauth::Authorize {
            client_id: registration.client_id.as_deref(),
            host_id: &registration.host_id,
            redirect_uri: &redirect_uri,
            state: &state,
            nonce: &nonce,
            challenge: &pkce.challenge,
        },
    )?;
    open_browser(&url)?;

    let callback = tokio::time::timeout(oauth::SIGN_IN_TIMEOUT, oauth::wait_for_callback(&listener))
        .await
        .map_err(|_| SplitError::Ai("The ChatGPT sign-in timed out. Try connecting again".into()))??;
    let code = oauth::code_from(&callback, &state)?;
    let client_id = registration
        .client_id
        .clone()
        .or(callback.client_id)
        .filter(|id| id != oauth::NEW_CLIENT)
        .ok_or_else(|| SplitError::Ai("OpenAI didn't register Splitr. Try connecting again".into()))?;
    registration.client_id = Some(client_id.clone());

    let http = oauth::client()?;
    let tokens = oauth::exchange(&http, &endpoints.token, &client_id, &code, &pkce.verifier, &redirect_uri).await?;
    let id_token = tokens.id_token.clone().ok_or_else(|| SplitError::Ai("ChatGPT didn't say which account signed in. Try again".into()))?;
    let keys = oauth::signing_keys(&http, &endpoints.discovery).await?;
    let payload = jwt::verify_signature(&id_token, &keys)?;
    let claims = jwt::check_claims(&payload, &endpoints.issuer, &client_id, &nonce, now())?;
    let account = Account { plan: claims.plan(), sub: claims.sub, email: claims.email, name: claims.name };
    Ok(Session::new(&client_id, tokens, account, None))
}

/// `session`, refreshed first if its access token expires within two minutes. Returns whether it
/// changed, so the caller saves it: refresh tokens rotate, so the old one no longer works.
pub async fn fresh(endpoints: &Endpoints, session: Session) -> Result<(Session, bool)> {
    if session.expires_at > now() + 120 {
        return Ok((session, false));
    }
    let refresh_token = session.refresh_token.clone().ok_or_else(|| SplitError::Ai(oauth::SIGNED_OUT.into()))?;
    let tokens = oauth::refresh(&oauth::client()?, &endpoints.token, &session.client_id, &refresh_token).await?;
    Ok((Session::new(&session.client_id.clone(), tokens, session.account.clone(), Some(&session)), true))
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::HashMap;

    use super::*;

    #[derive(Default)]
    struct MemoryStore(RefCell<HashMap<String, String>>);

    impl SecretStore for MemoryStore {
        fn get(&self, name: &str) -> Result<Option<String>> {
            Ok(self.0.borrow().get(name).cloned())
        }
        fn set(&self, name: &str, secret: &str) -> Result<()> {
            assert!(secret.encode_utf16().count() <= 1280, "{name} is too long for Windows");
            self.0.borrow_mut().insert(name.into(), secret.into());
            Ok(())
        }
        fn delete(&self, name: &str) -> Result<()> {
            self.0.borrow_mut().remove(name);
            Ok(())
        }
    }

    fn session(access: &str) -> Session {
        Session {
            client_id: "oaiapp_1".into(),
            access_token: access.into(),
            refresh_token: Some("rt-1".into()),
            expires_at: now() + 3600,
            scopes: oauth::SCOPES.split(' ').map(String::from).collect(),
            account: Account { sub: "u1".into(), email: Some("a@example.com".into()), name: None, plan: Some("plus".into()) },
        }
    }

    #[test]
    fn long_sessions_are_stored_in_parts_and_removed_completely() {
        let store = MemoryStore::default();
        save_session(&store, &session(&"a".repeat(3000))).unwrap();
        assert!(part_count(&store).unwrap() >= 4);
        assert_eq!(load_session(&store).unwrap().unwrap().access_token, "a".repeat(3000));
        // A shorter session replaces it and leaves no old parts behind.
        save_session(&store, &session("short")).unwrap();
        assert_eq!(store.0.borrow().len(), 2);
        delete_session(&store).unwrap();
        assert!(store.0.borrow().is_empty());
        assert!(load_session(&store).unwrap().is_none());
    }

    #[test]
    fn a_damaged_session_counts_as_signed_out() {
        let store = MemoryStore::default();
        store.set(ENTRY, "2").unwrap();
        store.set(&part(0), "{\"client_id\":").unwrap();
        assert!(load_session(&store).unwrap().is_none());
    }

    #[test]
    fn the_status_never_carries_tokens() {
        let status = Status::of(Some(&session("secret-access-token")));
        assert!(status.connected && status.plan_usage);
        let json = serde_json::to_string(&status).unwrap();
        assert!(!json.contains("secret-access-token") && !json.contains("rt-1"), "{json}");
        assert_eq!(Status::of(None), Status::default());
        let mut identity_only = session("x");
        identity_only.scopes = vec!["openid".into(), "email".into()];
        assert!(!Status::of(Some(&identity_only)).plan_usage);
    }

    #[test]
    fn the_registration_is_kept_and_a_new_install_gets_a_host_id() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub").join(FILE_NAME);
        let fresh = Registration::load(&path).unwrap();
        assert!(fresh.host_id.starts_with("urn:uuid:") && fresh.client_id.is_none());
        let saved = Registration { host_id: fresh.host_id.clone(), client_id: Some("oaiapp_1".into()) };
        saved.save(&path).unwrap();
        assert_eq!(Registration::load(&path).unwrap(), saved);
    }

    #[test]
    fn a_refresh_replaces_every_token_and_keeps_the_scopes_if_none_are_named() {
        let old = session("old");
        let tokens = oauth::Tokens {
            access_token: "new".into(),
            refresh_token: Some("rt-2".into()),
            id_token: None,
            expires_in: Some(60),
            scope: None,
        };
        let new = Session::new("oaiapp_1", tokens, old.account.clone(), Some(&old));
        assert_eq!((new.access_token.as_str(), new.refresh_token.as_deref()), ("new", Some("rt-2")));
        assert_eq!(new.scopes, old.scopes);
        assert!(new.expires_at <= now() + 60);
    }

    /// The whole sign-in against a local stand-in for OpenAI: the browser follows the authorize
    /// URL to the callback, the code is exchanged, and the ID token is checked.
    #[test]
    fn connect_signs_in_and_registers_splitr() {
        use crate::ai::http::test_server::serve;
        let (token_url, token_request) = serve(
            "/token",
            "200 OK",
            "",
            r#"{"access_token":"at","refresh_token":"rt","id_token":"not.a.jwt","expires_in":3600,"scope":"openid chatgpt.tokens.use.direct"}"#,
        );
        // Nothing listens on the discovery port, so the test makes no call outside this computer.
        let closed = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        let endpoints = Endpoints {
            token: token_url,
            authorize: "https://auth.example/authorize".into(),
            revoke: "https://auth.example/revoke".into(),
            discovery: format!("http://127.0.0.1:{closed}/.well-known/openid-configuration"),
            issuer: "https://auth.openai.com".into(),
        };
        let mut registration = Registration { host_id: "urn:uuid:test".into(), client_id: None };
        let result = tauri::async_runtime::block_on(connect(&endpoints, &mut registration, |url| {
            // The "browser": OpenAI redirects to the callback with a code and the issued client id.
            let p: HashMap<String, String> = reqwest::Url::parse(url).unwrap().query_pairs().into_owned().collect();
            assert_eq!(p["client_id"], oauth::NEW_CLIENT);
            assert_eq!(p["ext_agent_host_id"], "urn:uuid:test");
            let back = format!("{}?code=code-1&state={}&client_id=oaiapp_7", p["redirect_uri"], p["state"]);
            std::thread::spawn(move || {
                use std::io::{Read, Write};
                let addr = back.trim_start_matches("http://").split('/').next().unwrap().to_string();
                let path = &back[back.find("/auth").unwrap()..];
                let mut s = std::net::TcpStream::connect(addr).unwrap();
                write!(s, "GET {path} HTTP/1.1\r\n\r\n").unwrap();
                let _ = s.read_to_string(&mut String::new());
            });
            Ok(())
        }));
        // The token exchange used the issued client id and the PKCE verifier.
        let raw = token_request.recv().unwrap();
        assert!(raw.contains("client_id=oaiapp_7") && raw.contains("code=code-1") && raw.contains("code_verifier="), "{raw}");
        assert_eq!(registration.client_id.as_deref(), Some("oaiapp_7"));
        // The ID token here is fake, so the sign-in stops before trusting the account.
        assert!(result.is_err());
    }
}
