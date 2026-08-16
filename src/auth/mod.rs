//! OIDC (JWT Bearer) authentication.
//!
//! Validates an `Authorization: Bearer <token>` against the IdP's JWKS and
//! maps the token's claims to a user identity + groups, which the caller feeds
//! into `resolve_user_context` for role/RLS resolution.
//!
//! Design notes:
//! - Fails closed: any validation error returns `Err`, and callers map that to
//!   a deny-all context.
//! - The JWKS is fetched lazily and cached per `kid`; a key rotation triggers a
//!   single refresh (guarded by a mutex), so the request path never blocks on
//!   the network after the first token.
//! - Only RSA (RS256/384/512), EC (ES256/384), and HMAC (HS256/384/512) keys
//!   from the JWKS are supported; unknown key types are skipped.

use jsonwebtoken::{DecodingKey, Validation, decode, decode_header};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock, RwLock};

use crate::project::config::OidcConfig;

/// A verified identity extracted from a token.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedIdentity {
    pub user_id: String,
    pub groups: Vec<String>,
}

#[derive(Debug)]
pub enum AuthError {
    InvalidToken(String),
    UnknownKeyId(String),
    Fetch(String),
    EmptyUser,
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthError::InvalidToken(m) => write!(f, "invalid token: {m}"),
            AuthError::UnknownKeyId(k) => write!(f, "unknown key id: {k}"),
            AuthError::Fetch(m) => write!(f, "jwks fetch failed: {m}"),
            AuthError::EmptyUser => write!(f, "token has no user identity claim"),
        }
    }
}

impl std::error::Error for AuthError {}

/// JWKS cache: `kid` -> decoding key, refreshed lazily on a key miss.
pub struct JwksCache {
    keys: RwLock<HashMap<String, DecodingKey>>,
    refresh: Mutex<()>,
}

impl Default for JwksCache {
    fn default() -> Self {
        JwksCache {
            keys: RwLock::new(HashMap::new()),
            refresh: Mutex::new(()),
        }
    }
}

impl JwksCache {
    /// Return the decoding key for `kid`, fetching the JWKS if it isn't cached.
    fn key_for(&self, jwks_uri: &str, kid: &str) -> Result<DecodingKey, AuthError> {
        if let Some(k) = self.keys.read().unwrap().get(kid) {
            return Ok(k.clone());
        }
        let _guard = self.refresh.lock().unwrap();
        // Double-check after acquiring the refresh lock (another thread may have fetched).
        if let Some(k) = self.keys.read().unwrap().get(kid) {
            return Ok(k.clone());
        }
        let fetched = fetch_jwks(jwks_uri)?;
        let key = fetched.get(kid).cloned().ok_or_else(|| {
            let known: Vec<String> = fetched.keys().cloned().collect();
            AuthError::UnknownKeyId(format!("{kid} (known: {known:?})"))
        })?;
        *self.keys.write().unwrap() = fetched;
        Ok(key)
    }
}

/// Process-wide cache. Config is static per process, so a single cache is fine.
static CACHE: OnceLock<JwksCache> = OnceLock::new();

pub fn cache() -> &'static JwksCache {
    CACHE.get_or_init(JwksCache::default)
}

#[derive(Deserialize)]
struct Jwks {
    keys: Vec<Jwk>,
}

#[derive(Deserialize)]
struct Jwk {
    kty: String,
    #[serde(default)]
    kid: Option<String>,
    #[serde(default)]
    n: String,
    #[serde(default)]
    e: String,
    #[serde(default)]
    x: String,
    #[serde(default)]
    y: String,
    #[serde(default)]
    k: String,
}

fn fetch_jwks(jwks_uri: &str) -> Result<HashMap<String, DecodingKey>, AuthError> {
    let mut resp = ureq::get(jwks_uri)
        .call()
        .map_err(|e| AuthError::Fetch(e.to_string()))?;
    let body = resp
        .body_mut()
        .read_to_string()
        .map_err(|e| AuthError::Fetch(e.to_string()))?;
    let jwks: Jwks = serde_json::from_str(&body).map_err(|e| AuthError::Fetch(e.to_string()))?;
    jwks_to_keys(&jwks)
}

fn jwks_to_keys(jwks: &Jwks) -> Result<HashMap<String, DecodingKey>, AuthError> {
    let mut map = HashMap::new();
    for key in &jwks.keys {
        let decoding_key = match key.kty.as_str() {
            "RSA" => DecodingKey::from_rsa_components(&key.n, &key.e)
                .map_err(|e| AuthError::InvalidToken(e.to_string()))?,
            "EC" => DecodingKey::from_ec_components(&key.x, &key.y)
                .map_err(|e| AuthError::InvalidToken(e.to_string()))?,
            "oct" => DecodingKey::from_secret(key.k.as_bytes()),
            _ => continue,
        };
        if let Some(kid) = &key.kid {
            map.insert(kid.clone(), decoding_key);
        }
    }
    Ok(map)
}

/// OIDC discovery: resolve the JWKS URI from the issuer, cached per issuer.
fn resolve_jwks_uri(config: &OidcConfig) -> Result<String, AuthError> {
    if let Some(uri) = &config.jwks_uri {
        return Ok(uri.clone());
    }
    static DISCOVERY: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
    let map = DISCOVERY.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(uri) = map.lock().unwrap().get(&config.issuer) {
        return Ok(uri.clone());
    }
    let discovery_uri = format!("{}/.well-known/openid-configuration", config.issuer);
    let mut resp = ureq::get(&discovery_uri)
        .call()
        .map_err(|e| AuthError::Fetch(e.to_string()))?;
    let body = resp
        .body_mut()
        .read_to_string()
        .map_err(|e| AuthError::Fetch(e.to_string()))?;
    let doc: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| AuthError::Fetch(e.to_string()))?;
    let jwks_uri = doc
        .get("jwks_uri")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AuthError::Fetch("discovery response missing jwks_uri".into()))?
        .to_string();
    map.lock()
        .unwrap()
        .insert(config.issuer.clone(), jwks_uri.clone());
    Ok(jwks_uri)
}

/// Validate a Bearer token and extract the identity it represents.
pub fn validate_and_resolve(
    token: &str,
    config: &OidcConfig,
    cache: &JwksCache,
) -> Result<ResolvedIdentity, AuthError> {
    let header = decode_header(token).map_err(|e| AuthError::InvalidToken(e.to_string()))?;
    let alg = header.alg;
    let kid = header.kid.clone().unwrap_or_default();

    let jwks_uri = resolve_jwks_uri(config)?;
    let key = cache.key_for(&jwks_uri, &kid)?;

    let mut validation = Validation::new(alg);
    validation.set_issuer(&[&config.issuer]);
    validation.set_audience(&[&config.audience]);
    validation.validate_exp = true;
    validation.validate_nbf = true;

    let data = decode::<serde_json::Value>(token, &key, &validation)
        .map_err(|e| AuthError::InvalidToken(e.to_string()))?;
    extract_identity(&data.claims, config)
}

fn extract_identity(
    claims: &serde_json::Value,
    config: &OidcConfig,
) -> Result<ResolvedIdentity, AuthError> {
    let user_id = claims
        .get(&config.user_claim)
        .and_then(|v| v.as_str())
        .or_else(|| claims.get("sub").and_then(|v| v.as_str()))
        .unwrap_or("")
        .to_string();
    if user_id.is_empty() {
        return Err(AuthError::EmptyUser);
    }
    let mut groups = Vec::new();
    for claim in [&config.group_claim, &config.role_claim] {
        if let Some(name) = claim
            && let Some(v) = claims.get(name)
        {
            collect_strings(v, &mut groups);
        }
    }
    Ok(ResolvedIdentity { user_id, groups })
}

fn collect_strings(value: &serde_json::Value, out: &mut Vec<String>) {
    match value {
        serde_json::Value::Array(items) => {
            for item in items {
                if let Some(s) = item.as_str() {
                    out.push(s.to_string());
                }
            }
        }
        serde_json::Value::String(s) => out.push(s.clone()),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};

    // RSA-2048 test key (generated once for tests; do not use in production).
    const RSA_PRIVATE_PEM: &str = r#"-----BEGIN PRIVATE KEY-----
MIIEvgIBADANBgkqhkiG9w0BAQEFAASCBKgwggSkAgEAAoIBAQC7kQr8A293euHT
TW+PGzdLfmxzi09Uwe1rRpriNXtlpgRSyO77AjyUXXL691kOqPqX7SHjMVWovDmn
frrwKq//ZegV6MujO+S+oWi0kVYuK86HVrjwdCgSUcNusS+jSVX7zVlLvPldLuzB
3CnRq7spZj3ZFDTr1UjjFhB+t9sEgGnpfHWFEuuMdWXD8OtwnF++G0+Kq4Eqj4zd
hDPYM9QdT6MTDV+9icmVmMElpk6inKkew1LVRd+6wcVahMvcy7E6Cf8b5nje0T3G
IpmfJzwLbusbr8bJ0MkE0MPZp+IxuUDBkZ6BqHFuMG70VmGAZ9sckjoyccLIVZhe
MhkTICLbAgMBAAECggEAUcjMoVZeJBrQnPIG0r9rPN/DKh50WOC2RTBkGH55b7kT
6YTzrz8qawbUO9neWyYTHmunewjn8Msh2IbJvC7gztwAdo0rPeG/u99laFZ23Wr8
gsatnSsIzfQY4wsfWnN/qmu/o+aiVJ9BXMZC9cmLsmGCVkUZTjuRrHLSfm5scWlN
doOp19+JPJLk4zNKSpHlCE9PC2PaBlym53d/uwPrufSeKt8AMN5RYH+MkDgxru75
jHz4A8qStH2Ks6TA4BDsCZhIB1G56JAI0e7QPeke2x7fGEkf/4xspCqX3whLKfIy
gBfDxOIOxq0l0v15hR9jdQ6Eufd9XkepLlhErN4sAQKBgQDlswzHYpGRTwXjj1ji
QDIMv+zXCC1d0hsYsQoUTNzsgQURqsqqS27pfX0ys0sb7prrzoqXOcSKLzX58hfd
l+10LAY+2IxhIfBFUrcdzHUPxR1NEmJLMUpLRXkYIpvzPhwTPzCK04/RCmKekJt3
yzxy/ZROMCEKlk1eaWLHc7pRsQKBgQDRCv66Nw/R0WTjegoC5sY08awciOq9MrKL
Eh4JTfVgCF1FutkdFGA6+wB99wsy7QyOoEnkmFQf8gelIFTz9ZYsw3DQ5M4vS9cR
X7bcCBOrzoJnXF1Rqea0WoL+ekUtSH4/sU1DAuZITXrabAOdtz1j7KmyBlzWY0uH
KUIa0mZ0SwKBgAKWK5SrNXxvaV+Qo62Yj3e5SY96VhhyXz+97qEf5HT9VvNF+xZD
1zHl6d0CS9w6qZ/yKXleeyobMf5ojYA/T7s5K7DVe280lKITNmCthrvwuk294UF7
gpyqYZy19w+cKXDGC2Pk0f/GoCms8zM8JKge1uaygAzAeIqRoT0hvn1hAoGBAKpl
iP/nlCiWw+M8/l4hGN3dRUs5PAcfBTShfwRcnLA8ATOuu/2dN1e8dEk5j9JiMgMs
my8QEDq+AgdS1IzL2i8A3LwaVgttiZDq8VZn6wj324o/Wv4PPTQ0N2UR68OevPJU
J0OBYI79QTA8LbJoxEzog+bOkuxaoh05v123tbYDAoGBAM0Bfmm3m4xs5e2HHhsQ
PvTQSEhSFpaax9+6tZuRcGB1FweaDDXOIdSkETVCmiBJII4pmyxZx0vUWGeUjOhM
bY7/YgletW/9fXGefsxvFKh9rQPrYAGu9sQxOqxCV7HQt0cKidEuMbIlvIZy82eW
aBV5pIn0KS+wC44pVOWRpB9x
-----END PRIVATE KEY-----
"#;

    // Public key components (n, e) for the JWKS, base64url-encoded.
    const RSA_N: &str = "u5EK_ANvd3rh001vjxs3S35sc4tPVMHta0aa4jV7ZaYEUsju-wI8lF1y-vdZDqj6l-0h4zFVqLw5p3668Cqv_2XoFejLozvkvqFotJFWLivOh1a48HQoElHDbrEvo0lV-81ZS7z5XS7swdwp0au7KWY92RQ069VI4xYQfrfbBIBp6Xx1hRLrjHVlw_DrcJxfvhtPiquBKo-M3YQz2DPUHU-jEw1fvYnJlZjBJaZOopypHsNS1UXfusHFWoTL3MuxOgn_G-Z43tE9xiKZnyc8C27rG6_GydDJBNDD2afiMblAwZGegahxbjBu9FZhgGfbHJI6MnHCyFWYXjIZEyAi2w";
    const RSA_E: &str = "AQAB";

    fn oidc_config() -> OidcConfig {
        OidcConfig {
            issuer: "https://issuer.example.com".into(),
            audience: "test-client".into(),
            // A static URI avoids OIDC discovery (network) in tests; the key is
            // pre-populated in the cache by `with_jwks`.
            jwks_uri: Some("http://unused-jwks".into()),
            user_claim: "preferred_username".into(),
            group_claim: Some("groups".into()),
            role_claim: None,
        }
    }

    fn jwks_json(kid: &str) -> String {
        format!(r#"{{"keys":[{{"kty":"RSA","kid":"{kid}","n":"{RSA_N}","e":"{RSA_E}"}}]}}"#)
    }

    fn sign(kid: &str, claims: serde_json::Value, alg: Algorithm) -> String {
        let key = EncodingKey::from_rsa_pem(RSA_PRIVATE_PEM.as_bytes()).expect("rsa key");
        let mut header = Header::new(alg);
        header.kid = Some(kid.to_string());
        encode(&header, &claims, &key).expect("encode")
    }

    /// Pre-populate a cache with the test JWKS (no network needed).
    fn with_jwks(kid: &str, f: impl FnOnce(&JwksCache)) {
        let cache = JwksCache::default();
        let jwks: Jwks = serde_json::from_str(&jwks_json(kid)).expect("parse jwks");
        *cache.keys.write().unwrap() = jwks_to_keys(&jwks).expect("build keys");
        f(&cache);
    }

    fn valid_claims() -> serde_json::Value {
        serde_json::json!({
            "sub": "u-123",
            "preferred_username": "alice@example.com",
            "groups": ["Analysts", "Finance"],
            "iss": "https://issuer.example.com",
            "aud": "test-client",
            "exp": 2000000000, // 2033-05-18
        })
    }

    #[test]
    fn validates_rs256_token_and_extracts_identity() {
        with_jwks("key-1", |cache| {
            let t = sign("key-1", valid_claims(), Algorithm::RS256);
            let id = validate_and_resolve(&t, &oidc_config(), cache).expect("valid");
            assert_eq!(id.user_id, "alice@example.com");
            assert_eq!(id.groups, vec!["Analysts", "Finance"]);
        });
    }

    #[test]
    fn rejects_wrong_issuer() {
        with_jwks("key-1", |cache| {
            let mut claims = valid_claims();
            claims["iss"] = serde_json::json!("https://evil.example.com");
            let t = sign("key-1", claims, Algorithm::RS256);
            assert!(validate_and_resolve(&t, &oidc_config(), cache).is_err());
        });
    }

    #[test]
    fn rejects_wrong_audience() {
        with_jwks("key-1", |cache| {
            let mut claims = valid_claims();
            claims["aud"] = serde_json::json!("other-client");
            let t = sign("key-1", claims, Algorithm::RS256);
            assert!(validate_and_resolve(&t, &oidc_config(), cache).is_err());
        });
    }

    #[test]
    fn rejects_missing_identity() {
        with_jwks("key-1", |cache| {
            let mut claims = valid_claims();
            claims.as_object_mut().unwrap().remove("sub");
            claims.as_object_mut().unwrap().remove("preferred_username");
            let t = sign("key-1", claims, Algorithm::RS256);
            assert!(matches!(
                validate_and_resolve(&t, &oidc_config(), cache),
                Err(AuthError::EmptyUser)
            ));
        });
    }

    #[test]
    fn rejects_token_signed_by_different_key() {
        with_jwks("key-1", |cache| {
            let t = sign("key-other", valid_claims(), Algorithm::RS256);
            // "key-other" is not in the cache -> key lookup fails closed.
            assert!(validate_and_resolve(&t, &oidc_config(), cache).is_err());
        });
    }
}
