//! Secrets the server must hand back, such as the password of a camera, kept
//! sealed with a key of its own.
//!
//! A password of an account is never handed back, so it is kept in a form
//! nothing can turn back. A camera's has to reach the application that opens
//! the camera, so it is sealed instead: AES-256-GCM, with a key the server
//! draws once and keeps apart from the database. Each sealing draws a nonce of
//! its own, written before what it sealed, and is bound to what it belongs to,
//! so a sealed password moved onto another camera does not open.

use aws_lc_rs::aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_256_GCM, NONCE_LEN};

use crate::{AuthError, Result};

/// How many bytes a sealing key holds.
pub const KEY_BYTES: usize = 32;

/// The key secrets are sealed with.
pub struct SealingKey(LessSafeKey);

impl std::fmt::Debug for SealingKey {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("SealingKey(..)")
    }
}

impl SealingKey {
    /// A key drawn from the machine's randomness, to be kept.
    pub fn draw() -> Result<[u8; KEY_BYTES]> {
        let mut key = [0u8; KEY_BYTES];
        getrandom::fill(&mut key).map_err(|error| AuthError::NoRandomness(error.to_string()))?;
        Ok(key)
    }

    pub fn from_bytes(key: &[u8]) -> Result<Self> {
        UnboundKey::new(&AES_256_GCM, key)
            .map(|key| Self(LessSafeKey::new(key)))
            .map_err(|_| AuthError::NotASealingKey)
    }

    /// Seals `secret`, bound to `belongs_to`.
    pub fn seal(&self, secret: &str, belongs_to: &str) -> Result<Vec<u8>> {
        let mut nonce = [0u8; NONCE_LEN];
        getrandom::fill(&mut nonce).map_err(|error| AuthError::NoRandomness(error.to_string()))?;
        let mut sealed = secret.as_bytes().to_vec();
        self.0
            .seal_in_place_append_tag(
                Nonce::assume_unique_for_key(nonce),
                Aad::from(belongs_to.as_bytes()),
                &mut sealed,
            )
            .map_err(|_| AuthError::NotSealed)?;
        let mut kept = nonce.to_vec();
        kept.extend_from_slice(&sealed);
        Ok(kept)
    }

    /// Opens what [`Self::seal`] made for `belongs_to`.
    pub fn open(&self, sealed: &[u8], belongs_to: &str) -> Result<String> {
        if sealed.len() < NONCE_LEN {
            return Err(AuthError::NotSealed);
        }
        let (nonce, rest) = sealed.split_at(NONCE_LEN);
        let nonce = Nonce::try_assume_unique_for_key(nonce).map_err(|_| AuthError::NotSealed)?;
        let mut opened = rest.to_vec();
        let secret = self
            .0
            .open_in_place(nonce, Aad::from(belongs_to.as_bytes()), &mut opened)
            .map_err(|_| AuthError::NotSealed)?;
        String::from_utf8(secret.to_vec()).map_err(|_| AuthError::NotSealed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a_key() -> SealingKey {
        SealingKey::from_bytes(&SealingKey::draw().expect("drawn")).expect("a key")
    }

    #[test]
    fn a_sealed_secret_opens_with_its_key_for_what_it_belongs_to() {
        let key = a_key();
        let sealed = key.seal("a camera password", "camera-1").expect("sealed");
        assert!(
            !sealed.windows(6).any(|part| part == b"camera"),
            "nothing of the secret is kept as it was"
        );
        assert_eq!(key.open(&sealed, "camera-1").expect("opened"), "a camera password");
    }

    #[test]
    fn a_sealed_secret_opens_neither_elsewhere_nor_with_another_key() {
        let key = a_key();
        let sealed = key.seal("a camera password", "camera-1").expect("sealed");
        assert!(key.open(&sealed, "camera-2").is_err(), "moved onto another camera");
        assert!(a_key().open(&sealed, "camera-1").is_err(), "another server's key");
        let mut tampered = sealed.clone();
        *tampered.last_mut().expect("bytes") ^= 1;
        assert!(key.open(&tampered, "camera-1").is_err());
        assert!(key.open(&sealed[..5], "camera-1").is_err());
    }

    #[test]
    fn the_same_secret_sealed_twice_reads_differently() {
        let key = a_key();
        assert_ne!(key.seal("same", "camera-1").expect("sealed"), key.seal("same", "camera-1").expect("sealed"));
    }

    #[test]
    fn a_key_of_the_wrong_length_is_refused() {
        assert!(SealingKey::from_bytes(&[0u8; 16]).is_err());
    }
}
