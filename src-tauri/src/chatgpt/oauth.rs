//! The OAuth steps of Sign in with ChatGPT (#31): Authorization Code with PKCE (S256) in the
//! system browser, a one-time loopback callback on 127.0.0.1, and the token exchange and refresh.
//! Splitr never shows a login page and never sees the ChatGPT password.

use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use reqwest::Url;
use ring::digest::{SHA256, digest};
use ring::rand::{SecureRandom, SystemRandom};
use serde::Deserialize;
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::error::{Result, SplitError};

/// The client id that asks OpenAI to register Splitr for this user on the first sign-in.
pub const NEW_CLIENT: &str = "dynamic_agent_client";
/// The name OpenAI shows for Splitr on the consent page and in ChatGPT's settings.
const AGENT_NAME: &str = "Splitr";
/// Identity, then permission to use the ChatGPT plan for API requests.
pub const SCOPES: &str = "openid profile email offline_access resource.invoke chatgpt.tokens.use.direct";
/// The scope without which the account can't be used to split.
pub const PLAN_SCOPE: &str = "chatgpt.tokens.use.direct";
/// The API the tokens are for.
pub const RESOURCE: &str = "https://api.openai.com/v1";
pub const CALLBACK_PATH: &str = "/auth/callback";
/// How long the browser may take before the sign-in gives up.
pub const SIGN_IN_TIMEOUT: Duration = Duration::from_secs(300);

fn random(bytes: usize) -> Result<Vec<u8>> {
    let mut out = vec![0; bytes];
    SystemRandom::new()
        .fill(&mut out)
        .map_err(|_| SplitError::Ai("Splitr couldn't create a secure random value for the sign-in".into()))?;
    Ok(out)
}

/// A random URL-safe value with `bytes` bytes of randomness, for `state`, `nonce` and the verifier.
pub fn random_token(bytes: usize) -> Result<String> {
    Ok(URL_SAFE_NO_PAD.encode(random(bytes)?))
}

/// The PKCE verifier and its S256 challenge.
pub struct Pkce {
    pub verifier: String,
    pub challenge: String,
}

pub fn pkce() -> Result<Pkce> {
    let verifier = random_token(32)?;
    Ok(Pkce { challenge: challenge(&verifier), verifier })
}

fn challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(digest(&SHA256, verifier.as_bytes()))
}

/// A new opaque id for this installation (`ext_agent_host_id`): a random UUID, never anything
/// that identifies the user.
pub fn new_host_id() -> Result<String> {
    let mut b = random(16)?;
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let hex: String = b.iter().map(|x| format!("{x:02x}")).collect();
    Ok(format!("urn:uuid:{}-{}-{}-{}-{}", &hex[..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..]))
}

/// What goes into the authorize URL.
pub struct Authorize<'a> {
    /// The client id OpenAI issued earlier, or `None` to register.
    pub client_id: Option<&'a str>,
    pub host_id: &'a str,
    pub redirect_uri: &'a str,
    pub state: &'a str,
    pub nonce: &'a str,
    pub challenge: &'a str,
}

pub fn authorize_url(base: &str, a: &Authorize) -> Result<String> {
    let mut params = vec![
        ("client_id", a.client_id.unwrap_or(NEW_CLIENT)),
        ("ext_agent_host_id", a.host_id),
        ("response_type", "code"),
        ("redirect_uri", a.redirect_uri),
        ("scope", SCOPES),
        ("resource", RESOURCE),
        ("state", a.state),
        ("nonce", a.nonce),
        ("code_challenge_method", "S256"),
        ("code_challenge", a.challenge),
    ];
    if a.client_id.is_none() {
        params.push(("agent_name_hint", AGENT_NAME));
    }
    Url::parse_with_params(base, &params)
        .map(String::from)
        .map_err(|e| SplitError::Ai(format!("Splitr couldn't build the ChatGPT sign-in address ({e})")))
}

/// What the browser brought back to the loopback address.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Callback {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
    /// The client id OpenAI issued, on the first sign-in.
    pub client_id: Option<String>,
}

/// The callback's query parameters, if `target` (the request path) is the callback.
pub fn parse_callback(target: &str) -> Option<Callback> {
    let url = Url::parse(&format!("http://127.0.0.1{target}")).ok()?;
    if url.path() != CALLBACK_PATH {
        return None;
    }
    let mut cb = Callback::default();
    for (key, value) in url.query_pairs() {
        let value = Some(value.into_owned());
        match key.as_ref() {
            "code" => cb.code = value,
            "state" => cb.state = value,
            "error" => cb.error = value,
            "client_id" => cb.client_id = value,
            _ => {}
        }
    }
    Some(cb)
}

/// The authorization code, after checking `state` first. A user who declined gets a plain message.
pub fn code_from(cb: &Callback, state: &str) -> Result<String> {
    if cb.state.as_deref() != Some(state) {
        return Err(SplitError::Ai("The ChatGPT sign-in reply didn't match this sign-in. Try connecting again".into()));
    }
    if let Some(error) = &cb.error {
        return Err(SplitError::Ai(if error == "access_denied" {
            "ChatGPT wasn't connected: the permission was declined".into()
        } else {
            format!("ChatGPT couldn't connect ({error}). Try again")
        }));
    }
    cb.code.clone().filter(|c| !c.is_empty()).ok_or_else(|| SplitError::Ai("ChatGPT sent no sign-in code. Try again".into()))
}

const DONE_PAGE: &str = "<!doctype html><meta charset=utf-8><title>Splitr</title>\
    <body style=\"font:16px system-ui;margin:3em\"><h1>You can close this tab</h1>\
    <p>Go back to Splitr to finish connecting ChatGPT.</p>";

/// Waits on `listener` for the browser to reach the callback, answers it with a short page, and
/// returns what it brought. Other requests (such as a favicon) get a 404.
pub async fn wait_for_callback(listener: &TcpListener) -> Result<Callback> {
    loop {
        let (mut stream, _) = listener
            .accept()
            .await
            .map_err(|e| SplitError::Ai(format!("Splitr couldn't receive the ChatGPT sign-in ({e})")))?;
        let mut head = Vec::new();
        let mut buf = [0; 2048];
        while !head.windows(4).any(|w| w == b"\r\n\r\n") && head.len() < 16 * 1024 {
            match stream.read(&mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(n) => head.extend_from_slice(&buf[..n]),
            }
        }
        let text = String::from_utf8_lossy(&head);
        let target = text.lines().next().and_then(|line| line.strip_prefix("GET ")).and_then(|r| r.split(' ').next());
        let found = target.and_then(parse_callback);
        let reply = match found {
            Some(_) => format!(
                "HTTP/1.1 200 OK\r\ncontent-type: text/html; charset=utf-8\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{DONE_PAGE}",
                DONE_PAGE.len()
            ),
            None => "HTTP/1.1 404 Not Found\r\ncontent-length: 0\r\nconnection: close\r\n\r\n".into(),
        };
        let _ = stream.write_all(reply.as_bytes()).await;
        let _ = stream.shutdown().await;
        if let Some(cb) = found {
            return Ok(cb);
        }
    }
}

/// The token endpoint's reply. Deliberately not `Debug`: it holds the tokens.
#[derive(Deserialize)]
pub struct Tokens {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub id_token: Option<String>,
    #[serde(default)]
    pub expires_in: Option<u64>,
    #[serde(default)]
    pub scope: Option<String>,
}

/// An HTTPS client for OpenAI's sign-in endpoints.
pub fn client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .https_only(cfg!(not(test)))
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|e| SplitError::Ai(format!("Splitr couldn't set up its connection to ChatGPT ({e})")))
}

fn unreachable(e: &reqwest::Error) -> SplitError {
    if e.is_connect() || e.is_timeout() {
        SplitError::Ai("Splitr couldn't reach ChatGPT. Check your internet connection and try again".into())
    } else {
        SplitError::Ai(format!("The request to ChatGPT failed ({e})"))
    }
}

/// Why a refresh token can't be used any more, so the user must connect again.
pub const SIGNED_OUT: &str = "Your ChatGPT connection has ended. Connect ChatGPT again under AI in Settings";

/// POSTs `form` to the token endpoint at `url`.
async fn token_call(http: &reqwest::Client, url: &str, form: &[(&str, &str)]) -> Result<Tokens> {
    let response = http.post(url).form(form).send().await.map_err(|e| unreachable(&e))?;
    let status = response.status();
    let body = response.text().await.map_err(|e| unreachable(&e))?;
    if status.is_success() {
        return serde_json::from_str(&body).map_err(|_| SplitError::Ai("ChatGPT sent a sign-in reply Splitr couldn't read".into()));
    }
    let error = serde_json::from_str::<Value>(&body).ok();
    let code = error.as_ref().and_then(|v| v.get("error")).and_then(Value::as_str).unwrap_or("");
    Err(SplitError::Ai(match code {
        "invalid_grant" | "invalid_refresh_token" | "token_expired" | "refresh_token_expired"
        | "refresh_token_invalidated" | "refresh_token_reused" => SIGNED_OUT.into(),
        "invalid_client" => "OpenAI no longer recognizes Splitr's registration. Disconnect and connect ChatGPT again".into(),
        _ => format!("ChatGPT's sign-in failed (error {}). Try again", status.as_u16()),
    }))
}

/// Trades the authorization code for tokens.
pub async fn exchange(http: &reqwest::Client, url: &str, client_id: &str, code: &str, verifier: &str, redirect_uri: &str) -> Result<Tokens> {
    let form = [
        ("grant_type", "authorization_code"),
        ("client_id", client_id),
        ("code", code),
        ("code_verifier", verifier),
        ("redirect_uri", redirect_uri),
        ("resource", RESOURCE),
    ];
    token_call(http, url, &form).await
}

/// New tokens for a refresh token. Refresh tokens rotate: the reply carries the next one.
pub async fn refresh(http: &reqwest::Client, url: &str, client_id: &str, refresh_token: &str) -> Result<Tokens> {
    let form = [("grant_type", "refresh_token"), ("client_id", client_id), ("refresh_token", refresh_token)];
    token_call(http, url, &form).await
}

/// OpenAI's signing keys, from its OpenID configuration at `discovery`.
pub async fn signing_keys(http: &reqwest::Client, discovery: &str) -> Result<Value> {
    let get = |url: String| async move {
        let response = http.get(&url).send().await.map_err(|e| unreachable(&e))?;
        if !response.status().is_success() {
            return Err(SplitError::Ai(format!("ChatGPT's sign-in keys couldn't be read (error {})", response.status().as_u16())));
        }
        response.json::<Value>().await.map_err(|_| SplitError::Ai("ChatGPT's sign-in keys couldn't be read".into()))
    };
    let config = get(discovery.to_string()).await?;
    let jwks_uri = config
        .get("jwks_uri")
        .and_then(Value::as_str)
        .ok_or_else(|| SplitError::Ai("ChatGPT's sign-in settings don't list its keys".into()))?;
    get(jwks_uri.to_string()).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_uses_s256_of_the_verifier() {
        // RFC 7636, Appendix B.
        assert_eq!(challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"), "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
        let p = pkce().unwrap();
        assert_eq!(p.verifier.len(), 43);
        assert_eq!(p.challenge, challenge(&p.verifier));
        assert_ne!(pkce().unwrap().verifier, p.verifier);
    }

    #[test]
    fn host_ids_are_random_uuids() {
        let id = new_host_id().unwrap();
        assert_eq!(id.len(), "urn:uuid:".len() + 36);
        assert!(id.starts_with("urn:uuid:") && id.as_bytes()[9 + 14] == b'4', "{id}");
        assert_ne!(new_host_id().unwrap(), id);
    }

    fn params(url: &str) -> Vec<(String, String)> {
        Url::parse(url).unwrap().query_pairs().map(|(k, v)| (k.into_owned(), v.into_owned())).collect()
    }

    #[test]
    fn the_first_sign_in_registers_splitr_and_later_ones_reuse_its_client_id() {
        let a = Authorize {
            client_id: None,
            host_id: "urn:uuid:1",
            redirect_uri: "http://127.0.0.1:5000/auth/callback",
            state: "s",
            nonce: "n",
            challenge: "c",
        };
        let first = params(&authorize_url("https://auth.openai.com/api/accounts/authorize", &a).unwrap());
        let get = |p: &[(String, String)], k: &str| p.iter().find(|(key, _)| key == k).map(|(_, v)| v.clone());
        assert_eq!(get(&first, "client_id").as_deref(), Some(NEW_CLIENT));
        assert_eq!(get(&first, "agent_name_hint").as_deref(), Some("Splitr"));
        assert_eq!(get(&first, "scope").as_deref(), Some(SCOPES));
        assert_eq!(get(&first, "code_challenge_method").as_deref(), Some("S256"));
        assert_eq!(get(&first, "redirect_uri").as_deref(), Some("http://127.0.0.1:5000/auth/callback"));
        assert_eq!(get(&first, "resource").as_deref(), Some(RESOURCE));
        let later = params(&authorize_url("https://x/a", &Authorize { client_id: Some("oaiapp_1"), ..a }).unwrap());
        assert_eq!(get(&later, "client_id").as_deref(), Some("oaiapp_1"));
        assert_eq!(get(&later, "agent_name_hint"), None);
    }

    #[test]
    fn the_callback_is_read_and_its_state_checked_first() {
        let cb = parse_callback("/auth/callback?code=abc&state=s1&scope=openid&client_id=oaiapp_9").unwrap();
        assert_eq!(cb.client_id.as_deref(), Some("oaiapp_9"));
        assert_eq!(code_from(&cb, "s1").unwrap(), "abc");
        assert!(code_from(&cb, "s2").unwrap_err().to_string().contains("didn't match"));
        assert_eq!(parse_callback("/favicon.ico"), None);
        let denied = parse_callback("/auth/callback?error=access_denied&state=s1").unwrap();
        assert!(code_from(&denied, "s1").unwrap_err().to_string().contains("declined"));
        // A forged reply with another state is refused before its error is even read.
        let forged = parse_callback("/auth/callback?error=access_denied&state=x").unwrap();
        assert!(code_from(&forged, "s1").unwrap_err().to_string().contains("didn't match"));
    }

    #[test]
    fn the_loopback_server_answers_the_browser_and_ignores_other_requests() {
        use std::io::{Read, Write};
        tauri::async_runtime::block_on(async {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let port = listener.local_addr().unwrap().port();
            let browser = std::thread::spawn(move || {
                let get = |path: &str| {
                    let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
                    write!(s, "GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n").unwrap();
                    let mut reply = String::new();
                    s.read_to_string(&mut reply).unwrap();
                    reply
                };
                (get("/favicon.ico"), get("/auth/callback?code=c1&state=s1"))
            });
            let cb = wait_for_callback(&listener).await.unwrap();
            assert_eq!((cb.code.as_deref(), cb.state.as_deref()), (Some("c1"), Some("s1")));
            let (favicon, page) = browser.join().unwrap();
            assert!(favicon.starts_with("HTTP/1.1 404"));
            assert!(page.starts_with("HTTP/1.1 200") && page.contains("You can close this tab"));
        });
    }

    #[test]
    fn token_errors_say_what_to_do() {
        use crate::ai::http::test_server::serve;
        let http = client().unwrap();
        let (url, got) = serve("/token", "400 Bad Request", "", r#"{"error":"invalid_grant"}"#);
        let err = tauri::async_runtime::block_on(refresh(&http, &url, "oaiapp_1", "rt-secret")).err().unwrap();
        assert_eq!(err.to_string(), SIGNED_OUT);
        let raw = got.recv().unwrap();
        assert!(raw.contains("grant_type=refresh_token") && raw.contains("client_id=oaiapp_1"));
        let (url, _) = serve("/token", "200 OK", "", r#"{"access_token":"at","refresh_token":"rt2","expires_in":3600,"scope":"openid"}"#);
        let t = tauri::async_runtime::block_on(refresh(&http, &url, "oaiapp_1", "rt")).ok().unwrap();
        assert_eq!((t.access_token.as_str(), t.refresh_token.as_deref(), t.expires_in), ("at", Some("rt2"), Some(3600)));
    }
}
