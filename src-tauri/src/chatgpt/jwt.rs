//! Checks the ID token from a ChatGPT sign-in (#31): its RS256 signature against OpenAI's published
//! keys, then its issuer, audience, expiry and nonce. Only then is the account it names trusted.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use ring::signature::{RSA_PKCS1_2048_8192_SHA256, RsaPublicKeyComponents};
use serde::Deserialize;
use serde_json::Value;

use crate::error::{Result, SplitError};

/// What the ID token says about the account.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Claims {
    pub sub: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    nonce: Option<String>,
    iss: String,
    aud: Value,
    exp: u64,
    /// OpenAI's own claims, which may name the ChatGPT plan.
    #[serde(default, rename = "https://api.openai.com/auth")]
    auth: Option<Value>,
}

impl Claims {
    /// The ChatGPT plan ("plus", "pro"…), if the token names it.
    pub fn plan(&self) -> Option<String> {
        let plan = self.auth.as_ref()?.get("chatgpt_plan_type")?.as_str()?.trim();
        (!plan.is_empty()).then(|| plan.to_string())
    }
}

fn bad(why: &str) -> SplitError {
    SplitError::Ai(format!("ChatGPT's sign-in reply couldn't be trusted ({why}). Try connecting again"))
}

fn decode(part: &str) -> Result<Vec<u8>> {
    URL_SAFE_NO_PAD.decode(part).map_err(|_| bad("it isn't encoded correctly"))
}

/// Checks the signature of `token` with the matching RS256 key in `jwks` and returns its payload.
pub fn verify_signature(token: &str, jwks: &Value) -> Result<Vec<u8>> {
    let mut parts = token.split('.');
    let (Some(header), Some(payload), Some(signature), None) = (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(bad("it isn't a signed token"));
    };
    let head: Value = serde_json::from_slice(&decode(header)?).map_err(|_| bad("its header can't be read"))?;
    if head.get("alg").and_then(Value::as_str) != Some("RS256") {
        return Err(bad("it isn't signed with RS256"));
    }
    let kid = head.get("kid").and_then(Value::as_str);
    let keys = jwks.get("keys").and_then(Value::as_array).map(Vec::as_slice).unwrap_or_default();
    let key = keys
        .iter()
        .filter(|k| k.get("kty").and_then(Value::as_str) == Some("RSA"))
        .find(|k| kid.is_none() || k.get("kid").and_then(Value::as_str) == kid)
        .ok_or_else(|| bad("no published key matches it"))?;
    let field = |name: &str| key.get(name).and_then(Value::as_str).ok_or_else(|| bad("a published key is incomplete"));
    let public = RsaPublicKeyComponents { n: decode(field("n")?)?, e: decode(field("e")?)? };
    let signed = &token[..header.len() + 1 + payload.len()];
    public
        .verify(&RSA_PKCS1_2048_8192_SHA256, signed.as_bytes(), &decode(signature)?)
        .map_err(|_| bad("its signature doesn't match"))?;
    decode(payload)
}

/// Checks the claims of a verified payload: issued by `issuer` for `audience`, not expired at
/// `now` (Unix seconds), and carrying the `nonce` this sign-in sent.
pub fn check_claims(payload: &[u8], issuer: &str, audience: &str, nonce: &str, now: u64) -> Result<Claims> {
    let claims: Claims = serde_json::from_slice(payload).map_err(|_| bad("it doesn't name an account"))?;
    if claims.iss != issuer {
        return Err(bad("it comes from the wrong issuer"));
    }
    let for_us = match &claims.aud {
        Value::String(aud) => aud == audience,
        Value::Array(list) => list.iter().any(|a| a.as_str() == Some(audience)),
        _ => false,
    };
    if !for_us {
        return Err(bad("it was issued for another app"));
    }
    // A minute of leeway for a computer clock that is slightly off.
    if claims.exp + 60 < now {
        return Err(bad("it has expired"));
    }
    if claims.nonce.as_deref() != Some(nonce) {
        return Err(bad("it belongs to another sign-in"));
    }
    Ok(claims)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    // The RS256 example of RFC 7515, Appendix A.2: a public test vector, with only its public key.
    const RFC_TOKEN: &str = "eyJhbGciOiJSUzI1NiJ9\
        .eyJpc3MiOiJqb2UiLA0KICJleHAiOjEzMDA4MTkzODAsDQogImh0dHA6Ly9leGFtcGxlLmNvbS9pc19yb290Ijp0cnVlfQ\
        .cC4hiUPoj9Eetdgtv3hF80EGrhuB__dzERat0XF9g2VtQgr9PJbu3XOiZj5RZmh7AAuHIm4Bh-0Qc_lF5YKt_O8W2Fp5jujGbds9uJdbF9CUAr7t1dnZcAcQjbKBYNX4\
        BAynRFdiuB--f_nZLgrnbyTyWzO75vRK5h6xBArLIARNPvkSjtQBMHlb1L07Qe7K0GarZRmB_eSN9383LcOLn6_dO--xi12jzDwusC-eOkHWEsqtFZESc6BfI7noOPqv\
        hJ1phCnvWh6IeYI2w9QOYEUipUTI8np6LbgGY9Fs98rqVt5AXLIhWkWywlVmtVrBp0igcN_IoypGlUPQGe77Rw";
    const RFC_N: &str = "ofgWCuLjybRlzo0tZWJjNiuSfb4p4fAkd_wWJcyQoTbji9k0l8W26mPddxHmfHQp-Vaw-4qPCJrcS2mJPMEzP1Pt0Bm4d4QlL-yRT-SFd2lZS-pCgNMs\
        D1W_YpRPEwOWvG6b32690r2jZ47soMZo9wGzjb_7OMg0LOL-bSf63kpaSHSXndS5z5rexMdbBYUsLA9e-KXBdQOS-UTo7WTBEMa2R2CapHg665xsmtdV\
        MTBQY4uDZlxvb3qCo5ZwKh9kG4LT6_I5IhlJH7aGhyxXFvUK-DWNmoudF8NAco9_h9iaGNj8q2ethFkMLs91kzk2PAcDTW9gb54h4FRWyuXpoQ";

    fn jwks() -> Value {
        json!({ "keys": [{ "kty": "RSA", "n": RFC_N.replace(' ', ""), "e": "AQAB" }] })
    }

    fn token() -> String {
        RFC_TOKEN.split_whitespace().collect()
    }

    #[test]
    fn a_correctly_signed_token_gives_its_payload() {
        let payload = verify_signature(&token(), &jwks()).unwrap();
        let value: Value = serde_json::from_slice(&payload).unwrap();
        assert_eq!(value["iss"], "joe");
    }

    #[test]
    fn a_changed_token_or_a_wrong_key_is_refused() {
        let token = token();
        let mut parts: Vec<&str> = token.split('.').collect();
        // A different payload with the same signature.
        let forged = URL_SAFE_NO_PAD.encode(br#"{"iss":"joe","exp":9999999999}"#);
        parts[1] = &forged;
        let err = verify_signature(&parts.join("."), &jwks()).unwrap_err().to_string();
        assert!(err.contains("signature doesn't match"), "{err}");
        let other_key = json!({ "keys": [{ "kty": "RSA", "n": URL_SAFE_NO_PAD.encode([7u8; 256]), "e": "AQAB" }] });
        assert!(verify_signature(&token, &other_key).is_err());
        assert!(verify_signature(&token, &json!({ "keys": [] })).unwrap_err().to_string().contains("no published key"));
        // Unsigned ("alg": "none") tokens are never accepted.
        let none = format!("{}.{}.", URL_SAFE_NO_PAD.encode(br#"{"alg":"none"}"#), parts[1]);
        assert!(verify_signature(&none, &jwks()).unwrap_err().to_string().contains("RS256"));
        assert!(verify_signature("not-a-token", &jwks()).is_err());
    }

    fn payload(nonce: &str, aud: Value, exp: u64) -> Vec<u8> {
        serde_json::to_vec(&json!({
            "iss": "https://auth.openai.com", "aud": aud, "exp": exp, "sub": "user-1", "nonce": nonce,
            "email": "a@example.com", "name": "A. Person",
            "https://api.openai.com/auth": { "chatgpt_plan_type": "plus" },
        }))
        .unwrap()
    }

    #[test]
    fn claims_must_be_for_this_app_and_this_sign_in() {
        let iss = "https://auth.openai.com";
        let ok = check_claims(&payload("n1", json!("oaiapp_1"), 1000), iss, "oaiapp_1", "n1", 1000).unwrap();
        assert_eq!((ok.sub.as_str(), ok.email.as_deref(), ok.plan().as_deref()), ("user-1", Some("a@example.com"), Some("plus")));
        assert!(check_claims(&payload("n1", json!(["x", "oaiapp_1"]), 1000), iss, "oaiapp_1", "n1", 1000).is_ok());
        for (p, why) in [
            (payload("n1", json!("oaiapp_2"), 1000), "another app"),
            (payload("n2", json!("oaiapp_1"), 1000), "another sign-in"),
            (payload("n1", json!("oaiapp_1"), 100), "expired"),
        ] {
            let err = check_claims(&p, iss, "oaiapp_1", "n1", 1000).unwrap_err().to_string();
            assert!(err.contains(why), "{err}");
        }
        let err = check_claims(&payload("n1", json!("oaiapp_1"), 1000), "https://evil", "oaiapp_1", "n1", 1000).unwrap_err();
        assert!(err.to_string().contains("wrong issuer"));
    }
}
