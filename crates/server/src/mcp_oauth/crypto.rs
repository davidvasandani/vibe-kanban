use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::{RngCore, rngs::OsRng};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

/// An opaque bearer secret: `bytes` of OS randomness, base64url without padding.
pub fn random_secret(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    OsRng.fill_bytes(&mut buf);
    URL_SAFE_NO_PAD.encode(buf)
}

/// 256-bit secret for tokens, codes and consent nonces.
pub fn random_token() -> String {
    random_secret(32)
}

pub fn digest(secret: &str) -> Vec<u8> {
    Sha256::digest(secret.as_bytes()).to_vec()
}

pub fn digest_matches(secret: &str, expected: &[u8]) -> bool {
    digest(secret).as_slice().ct_eq(expected).unwrap_u8() == 1
}

/// RFC 7636 §4.1: 43–128 characters from the unreserved set.
pub fn is_valid_code_verifier(verifier: &str) -> bool {
    (43..=128).contains(&verifier.len())
        && verifier
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~'))
}

/// An S256 challenge is the base64url (no padding) SHA-256 digest: exactly 43
/// characters of the URL-safe alphabet.
pub fn is_valid_s256_challenge(challenge: &str) -> bool {
    challenge.len() == 43
        && challenge
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
}

pub fn s256_challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

pub fn pkce_matches(verifier: &str, challenge: &str) -> bool {
    is_valid_code_verifier(verifier)
        && s256_challenge(verifier)
            .as_bytes()
            .ct_eq(challenge.as_bytes())
            .unwrap_u8()
            == 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc7636_appendix_b_vector() {
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let challenge = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";
        assert_eq!(s256_challenge(verifier), challenge);
        assert!(pkce_matches(verifier, challenge));
        assert!(is_valid_s256_challenge(challenge));
        assert!(!pkce_matches("x".repeat(43).as_str(), challenge));
    }

    #[test]
    fn verifier_bounds_and_charset() {
        assert!(!is_valid_code_verifier(&"a".repeat(42)));
        assert!(is_valid_code_verifier(&"a".repeat(43)));
        assert!(is_valid_code_verifier(&"a".repeat(128)));
        assert!(!is_valid_code_verifier(&"a".repeat(129)));
        assert!(!is_valid_code_verifier(&format!("{}+", "a".repeat(42))));
    }

    #[test]
    fn secrets_are_random_and_digest_compares() {
        let a = random_token();
        assert_ne!(a, random_token());
        assert_eq!(a.len(), 43);
        assert!(digest_matches(&a, &digest(&a)));
        assert!(!digest_matches("other", &digest(&a)));
    }
}
