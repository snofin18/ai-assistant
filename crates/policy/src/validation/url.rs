//! Strict URL validation without a URL-parsing dependency.

use crate::{PolicyError, PolicyResult};

const MAX_URL_BYTES: usize = 4096;

/// URL validation policy supplied by the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UrlValidationRequest<'a> {
    /// URL supplied by the model.
    pub url: &'a str,
    /// Explicitly allowed schemes. Matches are ASCII case-insensitive.
    pub allowed_schemes: &'a [&'a str],
    /// Explicit allowed domains. A leading dot means suffix matching.
    pub allowed_hosts: &'a [&'a str],
}

/// A URL that passed strict parsing and allow-list checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedUrl {
    scheme: String,
    host: Option<String>,
    port: Option<u16>,
}

impl ValidatedUrl {
    /// Returns the lower-case scheme.
    #[must_use]
    pub fn scheme(&self) -> &str {
        &self.scheme
    }

    /// Returns the lower-case host for authority-based URLs.
    #[must_use]
    pub fn host(&self) -> Option<&str> {
        self.host.as_deref()
    }

    /// Returns the explicit port, when present.
    #[must_use]
    pub const fn port(&self) -> Option<u16> {
        self.port
    }
}

/// Validates a URL with fail-closed scheme and host allow-lists.
///
/// `file://` and direct IP hosts are always rejected, even if listed.
///
/// # Errors
///
/// Returns [`PolicyError::UrlRejected`] for malformed or disallowed URLs.
pub fn validate_url(request: &UrlValidationRequest<'_>) -> PolicyResult<ValidatedUrl> {
    let url = request.url;
    if url.is_empty() || url.len() > MAX_URL_BYTES {
        return Err(reject("URL is empty or exceeds the size limit"));
    }
    if url.chars().any(char::is_whitespace)
        || url.chars().any(char::is_control)
        || url.contains('\\')
        || url.contains('#')
    {
        return Err(reject(
            "URL contains whitespace, control, backslash, or fragment syntax",
        ));
    }
    let colon = url
        .find(':')
        .ok_or_else(|| reject("URL is missing a scheme separator"))?;
    let scheme = url
        .get(..colon)
        .ok_or_else(|| reject("URL scheme is malformed"))?
        .to_ascii_lowercase();
    let remainder = url
        .get(colon.saturating_add(1)..)
        .ok_or_else(|| reject("URL body is malformed"))?;
    if !is_valid_scheme(&scheme) {
        return Err(reject("URL scheme is malformed"));
    }
    if scheme == "file" {
        return Err(reject("file:// URLs are always forbidden"));
    }
    if !request
        .allowed_schemes
        .iter()
        .any(|allowed| allowed.eq_ignore_ascii_case(&scheme))
    {
        return Err(reject("URL scheme is not allowed"));
    }

    if scheme == "http" || scheme == "https" {
        parse_authority_url(&scheme, remainder, request.allowed_hosts)
    } else {
        if remainder.is_empty() || remainder.starts_with("//") {
            return Err(reject("custom-scheme URL body is empty or malformed"));
        }
        Ok(ValidatedUrl {
            scheme,
            host: None,
            port: None,
        })
    }
}

fn parse_authority_url(
    scheme: &str,
    remainder: &str,
    allowed_hosts: &[&str],
) -> PolicyResult<ValidatedUrl> {
    let authority_and_path = remainder
        .strip_prefix("//")
        .ok_or_else(|| reject("http/https URL requires an authority"))?;
    let authority_end = authority_and_path
        .find(['/', '?'])
        .unwrap_or(authority_and_path.len());
    let authority = authority_and_path
        .get(..authority_end)
        .ok_or_else(|| reject("URL authority is malformed"))?;
    if authority.is_empty()
        || authority.contains('@')
        || authority.contains('%')
        || authority.contains('[')
        || authority.contains(']')
    {
        return Err(reject(
            "URL authority is empty or contains unsupported syntax",
        ));
    }
    let (host, port) = split_host_port(authority)?;
    let host = host.to_ascii_lowercase();
    if host == "localhost"
        || is_decimal_host(&host)
        || is_hex_ip_literal(&host)
        || host.contains(':')
    {
        return Err(reject(
            "URL host must be a domain name, not localhost or a numeric IP literal",
        ));
    }
    if !is_valid_domain(&host) {
        return Err(reject("URL host is not a valid domain name"));
    }
    if allowed_hosts.is_empty()
        || !allowed_hosts
            .iter()
            .any(|allowed| host_matches(&host, allowed))
    {
        return Err(reject("URL host is not in the allow-list"));
    }
    Ok(ValidatedUrl {
        scheme: scheme.to_owned(),
        host: Some(host),
        port,
    })
}

fn split_host_port(authority: &str) -> PolicyResult<(&str, Option<u16>)> {
    let mut pieces = authority.split(':');
    let host = pieces.next().ok_or_else(|| reject("URL host is missing"))?;
    let port = match pieces.next() {
        None => None,
        Some(value) => {
            if pieces.next().is_some() || value.is_empty() {
                return Err(reject("IPv6 literals and empty ports are unsupported"));
            }
            let port = value
                .parse::<u16>()
                .map_err(|_| reject("URL port is invalid"))?;
            if port == 0 {
                return Err(reject("URL port zero is forbidden"));
            }
            Some(port)
        }
    };
    Ok((host, port))
}

fn host_matches(host: &str, allowed: &str) -> bool {
    let allowed = allowed.trim().to_ascii_lowercase();
    allowed.strip_prefix('.').map_or_else(
        || host == allowed,
        |suffix| host.len() > suffix.len() && host.ends_with(suffix),
    )
}

fn is_valid_scheme(value: &str) -> bool {
    let mut characters = value.chars();
    characters
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic())
        && characters.all(|character| {
            character.is_ascii_alphanumeric()
                || character == '+'
                || character == '-'
                || character == '.'
        })
}

fn is_valid_domain(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 253
        && value.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && label
                    .as_bytes()
                    .first()
                    .is_some_and(u8::is_ascii_alphanumeric)
                && label
                    .as_bytes()
                    .last()
                    .is_some_and(u8::is_ascii_alphanumeric)
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
}

fn is_decimal_host(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|character| character.is_ascii_digit() || character == '.')
}

fn is_hex_ip_literal(value: &str) -> bool {
    value.split('.').any(|label| {
        let Some(digits) = label
            .strip_prefix("0x")
            .or_else(|| label.strip_prefix("0X"))
        else {
            return false;
        };
        !digits.is_empty()
            && digits
                .chars()
                .all(|character| character.is_ascii_hexdigit())
    })
}

fn reject(reason: &str) -> PolicyError {
    PolicyError::UrlRejected {
        reason: reason.to_owned(),
    }
}
