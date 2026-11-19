//! The server's access key: generation, keyring storage and checking.
//!
//! The key never goes to SQLite, environment variables or the log (security
//! invariant 1). Clients receive it inside the connection code.

/// Keyring service and account of the server key.
const KEYRING_SERVICE: &str = "Glagol";
const KEYRING_USER: &str = "lan_server_key";

/// A new random key: 64 hex characters from two v4 UUIDs (244 random bits
/// from the operating system's CSPRNG).
pub fn generate_key() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

fn entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER).map_err(|e| e.to_string())
}

/// The stored key, creating and storing one on first use.
pub fn load_or_create_key() -> Result<String, String> {
    let entry = entry()?;
    match entry.get_password() {
        Ok(key) if !key.trim().is_empty() => Ok(key),
        Ok(_) | Err(keyring::Error::NoEntry) => {
            let key = generate_key();
            entry.set_password(&key).map_err(|e| e.to_string())?;
            Ok(key)
        }
        Err(e) => Err(e.to_string()),
    }
}

/// Replace the stored key; clients with the old key are refused afterwards.
pub fn regenerate_key() -> Result<String, String> {
    let key = generate_key();
    entry()?.set_password(&key).map_err(|e| e.to_string())?;
    Ok(key)
}

/// Whether an `Authorization` header carries `Bearer <key>`. The comparison
/// takes the same time wherever the first difference is.
pub fn authorized(header: Option<&str>, key: &str) -> bool {
    let Some(token) = header
        .and_then(|h| h.strip_prefix("Bearer "))
        .map(str::trim)
    else {
        return false;
    };
    let (a, b) = (token.as_bytes(), key.as_bytes());
    if a.len() != b.len() || key.is_empty() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_long_random_hex() {
        let (a, b) = (generate_key(), generate_key());
        assert_eq!(a.len(), 64);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
    }

    #[test]
    fn only_the_exact_bearer_key_is_authorized() {
        let key = generate_key();
        assert!(authorized(Some(&format!("Bearer {key}")), &key));
        assert!(!authorized(None, &key));
        assert!(!authorized(Some(&key), &key), "scheme required");
        assert!(!authorized(Some("Bearer "), &key));
        assert!(!authorized(Some(&format!("Bearer {key}x")), &key));
        let mut wrong = key.clone().into_bytes();
        wrong[63] = if wrong[63] == b'0' { b'1' } else { b'0' };
        let wrong = String::from_utf8(wrong).unwrap();
        assert!(!authorized(Some(&format!("Bearer {wrong}")), &key));
        assert!(!authorized(Some("Bearer "), ""), "empty key never matches");
    }
}
