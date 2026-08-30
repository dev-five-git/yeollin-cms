//! Opaque refresh-token generation and storage hashing.

use sha2::{Digest, Sha256};

/// Number of random bytes behind a refresh token.
const REFRESH_TOKEN_BYTES: usize = 32;

fn to_hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes.iter().fold(String::new(), |mut out, byte| {
        let _ = write!(out, "{byte:02x}");
        out
    })
}

/// Mint a refresh token.
///
/// Deliberately opaque rather than a JWT: the server must be able to revoke a
/// refresh token, which a self-contained signed token cannot express.
pub fn generate_refresh_token() -> String {
    to_hex(&rand::random::<[u8; REFRESH_TOKEN_BYTES]>())
}

/// Hash a refresh token for storage and lookup.
///
/// SHA-256 rather than Argon2: the input is 256 bits of uniform randomness, so
/// there is nothing to brute-force, and lookup requires a deterministic digest.
/// A database leak still exposes no usable token.
pub fn hash_refresh_token(token: &str) -> String {
    to_hex(&Sha256::digest(token.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_unpredictable_and_hex() {
        let first = generate_refresh_token();
        let second = generate_refresh_token();

        assert_eq!(first.len(), REFRESH_TOKEN_BYTES * 2);
        assert_ne!(first, second);
        assert!(first.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn hashing_is_deterministic_and_hides_the_token() {
        let token = generate_refresh_token();

        assert_eq!(hash_refresh_token(&token), hash_refresh_token(&token));
        assert_ne!(hash_refresh_token(&token), token);
        assert_eq!(hash_refresh_token(&token).len(), 64);
    }

    #[test]
    fn distinct_tokens_hash_differently() {
        assert_ne!(hash_refresh_token("a"), hash_refresh_token("b"));
    }
}
