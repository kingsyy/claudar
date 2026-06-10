//! Encryption-at-rest for session files.
//!
//! Session data is encrypted with AES-256-GCM. The key is stored in the OS
//! keychain (macOS Keychain / Linux Secret Service / Windows Credential
//! Manager) when available, falling back to a `0600` key file alongside the
//! encrypted data otherwise.

use aes_gcm::aead::{Aead, AeadCore, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::path::Path;

const SERVICE_NAME: &str = "claudar";
const KEY_ACCOUNT: &str = "session-encryption-key";
const FALLBACK_KEY_FILENAME: &str = ".master.key";
const ENVELOPE_VERSION: u8 = 1;

/// On-disk representation of an encrypted session file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Envelope {
    pub version: u8,
    pub nonce: String,
    pub ciphertext: String,
}

/// Get the AES-256 key used to encrypt/decrypt session files, generating and
/// persisting one if it doesn't exist yet.
pub fn get_or_create_key(fallback_dir: &Path) -> Result<[u8; 32]> {
    // Skip the OS keychain in unit tests so `cargo test` never prompts for
    // keychain access or touches the developer's real credential store.
    #[cfg(not(test))]
    {
        if let Ok(key) = try_keychain_key() {
            return Ok(key);
        }
    }

    get_or_create_fallback_key(fallback_dir)
}

#[cfg(not(test))]
fn try_keychain_key() -> Result<[u8; 32]> {
    let entry = keyring::Entry::new(SERVICE_NAME, KEY_ACCOUNT)?;
    match entry.get_password() {
        Ok(encoded) => decode_key(&encoded),
        Err(keyring::Error::NoEntry) => {
            let key = generate_key();
            entry.set_password(&STANDARD.encode(key))?;
            Ok(key)
        }
        Err(e) => Err(e.into()),
    }
}

fn get_or_create_fallback_key(dir: &Path) -> Result<[u8; 32]> {
    let path = dir.join(FALLBACK_KEY_FILENAME);
    if path.exists() {
        let encoded = std::fs::read_to_string(&path)?;
        decode_key(encoded.trim())
    } else {
        let key = generate_key();
        std::fs::create_dir_all(dir)?;
        std::fs::write(&path, STANDARD.encode(key))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        }
        Ok(key)
    }
}

fn generate_key() -> [u8; 32] {
    let mut key = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut key);
    key
}

fn decode_key(encoded: &str) -> Result<[u8; 32]> {
    let bytes = STANDARD
        .decode(encoded.trim())
        .context("encryption key is not valid base64")?;
    bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("encryption key has unexpected length"))
}

/// Encrypt `plaintext` with AES-256-GCM using a freshly generated nonce.
pub fn encrypt(plaintext: &[u8], key: &[u8; 32]) -> Result<Envelope> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ciphertext = cipher
        .encrypt(&nonce, plaintext)
        .map_err(|e| anyhow::anyhow!("encryption failed: {e}"))?;

    Ok(Envelope {
        version: ENVELOPE_VERSION,
        nonce: STANDARD.encode(nonce),
        ciphertext: STANDARD.encode(ciphertext),
    })
}

/// Decrypt an [`Envelope`], verifying its authentication tag.
pub fn decrypt(envelope: &Envelope, key: &[u8; 32]) -> Result<Vec<u8>> {
    anyhow::ensure!(
        envelope.version == ENVELOPE_VERSION,
        "unsupported session envelope version: {}",
        envelope.version
    );

    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let nonce_bytes = STANDARD
        .decode(&envelope.nonce)
        .context("invalid nonce encoding")?;
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = STANDARD
        .decode(&envelope.ciphertext)
        .context("invalid ciphertext encoding")?;

    cipher
        .decrypt(nonce, ciphertext.as_ref())
        .map_err(|e| anyhow::anyhow!("decryption failed: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypt_decrypt_round_trip() {
        let key = generate_key();
        let plaintext = b"super secret session data";

        let envelope = encrypt(plaintext, &key).unwrap();
        let decrypted = decrypt(&envelope, &key).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn decrypt_with_wrong_key_fails() {
        let key = generate_key();
        let other_key = generate_key();
        let envelope = encrypt(b"secret", &key).unwrap();

        assert!(decrypt(&envelope, &other_key).is_err());
    }

    #[test]
    fn fallback_key_persists_across_calls() {
        let dir = tempfile::tempdir().unwrap();

        let key1 = get_or_create_fallback_key(dir.path()).unwrap();
        let key2 = get_or_create_fallback_key(dir.path()).unwrap();

        assert_eq!(key1, key2);
    }
}
