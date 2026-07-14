//! Pure validators for the two user-supplied STT connection strings: the
//! provider `base_url` and the optional `proxy`.
//!
//! Both are plain `&str -> Result<_, String>` functions with table-driven
//! tests. The `Err` strings are user-facing Russian (the Settings form shows
//! them inline), matching the project convention of translating at the
//! boundary rather than surfacing internal English.

/// Hosts for which `http://` (no TLS) is acceptable — a locally-hosted
/// whisper server (Vox-Box / faster-whisper / Speaches) binds
/// `http://localhost:8000/v1` and never leaves the machine.
const LOCAL_HOSTS: [&str; 3] = ["localhost", "127.0.0.1", "::1"];

/// Validate a provider `base_url`.
///
/// Rules (security-critical, see `stt/mod.rs` D3):
/// - must parse as a URL with an `http`/`https` scheme;
/// - `https://` is always allowed;
/// - `http://` is allowed **only** when the host is loopback
///   (`localhost`/`127.0.0.1`/`::1`) — otherwise the Bearer API key would
///   travel in cleartext to a remote host, so it is rejected.
pub fn validate_base_url(raw: &str) -> Result<(), String> {
    let s = raw.trim();
    if s.is_empty() {
        return Err("Адрес не может быть пустым.".to_string());
    }

    let url = reqwest::Url::parse(s)
        .map_err(|_| "Некорректный адрес. Пример: https://api.aitunnel.ru/v1".to_string())?;

    let scheme = url.scheme();
    if scheme != "http" && scheme != "https" {
        return Err(format!(
            "Схема «{scheme}» не поддерживается — используйте https:// (или http:// для localhost)."
        ));
    }

    let host = normalized_host(&url).ok_or_else(|| "В адресе не указан хост.".to_string())?;

    if scheme == "http" && !is_local_host(&host) {
        return Err(
            "Для внешнего адреса разрешён только https:// — иначе ключ уйдёт открытым текстом. \
             http:// допустим только для локального сервера (localhost)."
                .to_string(),
        );
    }

    Ok(())
}

/// Validate an optional proxy string, returning `Ok(())` on success. This is a
/// thin wrapper over [`normalize_proxy`] for call sites that only need the
/// yes/no answer (e.g. live form validation).
pub fn validate_proxy(raw: &str) -> Result<(), String> {
    normalize_proxy(raw).map(|_| ())
}

/// Validate **and normalise** a proxy string into a fully-schemed URL suitable
/// for [`reqwest::Proxy::all`].
///
/// Accepts, per D5:
/// - `host:port`                     → prefixed with `http://`
/// - `login:pass@host:port`          → prefixed with `http://`, auth preserved
/// - `http://…` / `https://…` / `socks5://…` / `socks5h://…` → taken as-is
///
/// A host **and** an explicit port are required; any other scheme is rejected.
pub fn normalize_proxy(raw: &str) -> Result<String, String> {
    let s = raw.trim();
    if s.is_empty() {
        return Err("Прокси не может быть пустым.".to_string());
    }

    // No scheme given → default to http (the common "host:port" form).
    let candidate = if s.contains("://") {
        s.to_string()
    } else {
        format!("http://{s}")
    };

    let url = reqwest::Url::parse(&candidate).map_err(|_| {
        "Некорректный прокси. Примеры: host:port, login:pass@host:port, socks5://host:port"
            .to_string()
    })?;

    match url.scheme() {
        "http" | "https" | "socks5" | "socks5h" => {}
        other => {
            return Err(format!(
                "Схема прокси «{other}» не поддерживается (только http, https, socks5)."
            ));
        }
    }

    if normalized_host(&url).is_none() {
        return Err("В адресе прокси не указан хост.".to_string());
    }

    // Require an explicit port. `url.port()` cannot be used here: it returns
    // `None` when the port equals the scheme's default (`:80`/`:443`), which
    // would wrongly reject `https://proxy:443`. Scan the authority instead.
    if !authority_has_explicit_port(&candidate) {
        return Err("В адресе прокси не указан порт (например host:8080).".to_string());
    }

    Ok(candidate)
}

/// True when the `host:port` authority of `candidate` carries an explicit
/// numeric port. Handles an optional `scheme://`, `userinfo@` prefix, a
/// trailing path, and IPv6 literals in `[…]:port` form.
fn authority_has_explicit_port(candidate: &str) -> bool {
    let after_scheme = candidate
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(candidate);
    let authority = after_scheme
        .split(['/', '?', '#'])
        .next()
        .unwrap_or(after_scheme);
    let host_port = authority
        .rsplit_once('@')
        .map(|(_, hp)| hp)
        .unwrap_or(authority);

    // IPv6 literal: the port (if any) follows the closing bracket.
    if let Some(rest) = host_port.strip_prefix('[') {
        return match rest.split_once(']') {
            Some((_, tail)) => is_colon_port(tail),
            None => false,
        };
    }

    // host:port — the last colon must be followed by digits only.
    matches!(host_port.rsplit_once(':'), Some((_, port)) if is_port_digits(port))
}

fn is_colon_port(tail: &str) -> bool {
    tail.strip_prefix(':').is_some_and(is_port_digits)
}

fn is_port_digits(port: &str) -> bool {
    !port.is_empty() && port.chars().all(|c| c.is_ascii_digit())
}

/// Extract the host as a lowercase string without IPv6 brackets, returning
/// `None` for an empty host.
fn normalized_host(url: &reqwest::Url) -> Option<String> {
    let host = url.host_str()?;
    let stripped = host
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_ascii_lowercase();
    if stripped.is_empty() {
        None
    } else {
        Some(stripped)
    }
}

fn is_local_host(host: &str) -> bool {
    LOCAL_HOSTS.contains(&host)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_base_url_table() {
        // (input, expect_ok, note)
        let cases: &[(&str, bool)] = &[
            // https is always fine
            ("https://api.aitunnel.ru/v1", true),
            ("https://api.groq.com/openai/v1", true),
            // http allowed for loopback hosts
            ("http://localhost:8000/v1", true),
            ("http://127.0.0.1:8000/v1", true),
            ("http://[::1]:8000/v1", true),
            // https to a loopback host is fine too
            ("https://localhost:8000/v1", true),
            // http to an EXTERNAL host is rejected (cleartext key)
            ("http://api.aitunnel.ru/v1", false),
            ("http://example.com/v1", false),
            // wrong scheme
            ("ftp://example.com", false),
            ("ws://localhost:8000", false),
            // junk
            ("", false),
            ("   ", false),
            ("not a url", false),
            ("api.aitunnel.ru/v1", false), // no scheme
        ];

        for (input, expect_ok) in cases {
            let got = validate_base_url(input).is_ok();
            assert_eq!(
                got, *expect_ok,
                "validate_base_url({input:?}) expected ok={expect_ok}, got ok={got}"
            );
        }
    }

    #[test]
    fn validate_base_url_http_external_message_mentions_https() {
        let err = validate_base_url("http://example.com/v1").unwrap_err();
        assert!(
            err.contains("https"),
            "external http rejection should point at https, got: {err}"
        );
    }

    #[test]
    fn normalize_proxy_table() {
        // (input, expected_normalized_or_none_if_error)
        let cases: &[(&str, Option<&str>)] = &[
            // bare host:port gets http:// prefix
            (
                "proxy.example.com:8080",
                Some("http://proxy.example.com:8080"),
            ),
            // auth preserved through the prefix
            (
                "user:pass@proxy.example.com:8080",
                Some("http://user:pass@proxy.example.com:8080"),
            ),
            // explicit schemes pass through untouched
            (
                "http://proxy.example.com:3128",
                Some("http://proxy.example.com:3128"),
            ),
            (
                "https://proxy.example.com:443",
                Some("https://proxy.example.com:443"),
            ),
            ("socks5://127.0.0.1:1080", Some("socks5://127.0.0.1:1080")),
            ("socks5h://127.0.0.1:1080", Some("socks5h://127.0.0.1:1080")),
            // missing port
            ("proxy.example.com", None),
            ("http://proxy.example.com", None),
            // unsupported scheme
            ("ftp://proxy.example.com:21", None),
            // junk
            ("", None),
            ("   ", None),
        ];

        for (input, expected) in cases {
            match expected {
                Some(want) => {
                    let got = normalize_proxy(input)
                        .unwrap_or_else(|e| panic!("normalize_proxy({input:?}) errored: {e}"));
                    assert_eq!(&got, want, "normalize_proxy({input:?})");
                }
                None => {
                    assert!(
                        normalize_proxy(input).is_err(),
                        "normalize_proxy({input:?}) should be rejected"
                    );
                }
            }
        }
    }

    #[test]
    fn validate_proxy_delegates_to_normalize() {
        assert!(validate_proxy("host:8080").is_ok());
        assert!(validate_proxy("host-without-port").is_err());
    }
}
