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

// Used only by the release keychain path; unreferenced in debug/test builds.
#[cfg_attr(any(test, debug_assertions), allow(dead_code))]
const SERVICE_NAME: &str = "claudar";
#[cfg_attr(any(test, debug_assertions), allow(dead_code))]
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
///
/// The resolved key is cached for the lifetime of the process. The monitor
/// polls on an interval and each poll loads the session file; without the cache
/// every poll would re-hit the OS keychain and macOS would pop a fresh
/// permission prompt every cycle.
pub fn get_or_create_key(fallback_dir: &Path) -> Result<[u8; 32]> {
    // Skip the OS keychain in unit tests so `cargo test` never prompts for
    // keychain access or touches the developer's real credential store.
    #[cfg(test)]
    {
        get_or_create_fallback_key(fallback_dir)
    }
    #[cfg(not(test))]
    {
        cached_key(fallback_dir)
    }
}

#[cfg(not(test))]
fn cached_key(fallback_dir: &Path) -> Result<[u8; 32]> {
    use std::sync::OnceLock;
    static CACHE: OnceLock<[u8; 32]> = OnceLock::new();
    if let Some(key) = CACHE.get() {
        return Ok(*key);
    }
    // Only successful resolutions are cached; a transient failure is retried on
    // the next call rather than being pinned for the whole process.
    let key = resolve_key(fallback_dir)?;
    let _ = CACHE.set(key);
    Ok(key)
}

#[cfg(not(test))]
fn resolve_key(fallback_dir: &Path) -> Result<[u8; 32]> {
    // Dev builds run from unsigned / ad-hoc-signed binaries whose code signature
    // changes on every rebuild. macOS ties keychain ACLs to that signature, so
    // the keychain re-prompts forever and "Always Allow" never sticks. Skip the
    // keychain entirely in debug builds and use a stable on-disk key.
    #[cfg(debug_assertions)]
    {
        get_or_create_fallback_key(fallback_dir)
    }
    #[cfg(not(debug_assertions))]
    {
        resolve_release_key(fallback_dir)
    }
}

/// Release-build key resolution. Reads our single named keychain item and — this
/// is the important invariant — **never mints a fresh key just because a read
/// failed**. A brand-new key can only decrypt nothing, so silently generating
/// one on a keychain hiccup permanently orphans every existing session file.
#[cfg(all(not(test), not(debug_assertions)))]
fn resolve_release_key(fallback_dir: &Path) -> Result<[u8; 32]> {
    // Scope is deliberately a single named item: service "claudar", account
    // "session-encryption-key". `Entry` addresses exactly that one credential —
    // it does NOT enumerate, scan, or read any other keychain entry, and the OS
    // permission prompt names this item so the user can see what is being asked
    // for. We only ever read our own key or create it if absent.
    let entry =
        keyring::Entry::new(SERVICE_NAME, KEY_ACCOUNT).context("failed to open keychain entry")?;
    match entry.get_password() {
        Ok(encoded) => decode_key(&encoded),
        // Genuine first run on a reachable store: create and persist the key.
        Err(keyring::Error::NoEntry) => {
            let key = generate_key();
            entry
                .set_password(&STANDARD.encode(key))
                .context("failed to store new encryption key in keychain")?;
            Ok(key)
        }
        // The store exists but our key could not be read (locked, access denied,
        // signature/ACL mismatch, ...). Do NOT create a new key here.
        Err(e) => reuse_existing_or_fail(fallback_dir, e),
    }
}

#[cfg(all(not(test), not(debug_assertions)))]
fn reuse_existing_or_fail(fallback_dir: &Path, err: keyring::Error) -> Result<[u8; 32]> {
    // If a fallback key file was already established, it is a real prior key —
    // reuse it. This is the only safe on-disk path: it never invents a new key.
    let path = fallback_dir.join(FALLBACK_KEY_FILENAME);
    if path.exists() {
        let encoded = std::fs::read_to_string(&path)?;
        return decode_key(encoded.trim());
    }

    // No established on-disk key either. On platforms with no credential store
    // at all — e.g. a headless Linux box with no Secret Service — this is a
    // legitimate first run, so creating a file key is correct. macOS always has
    // a keychain, so an unreadable key there is a real access problem: surface
    // it rather than silently rekeying and destroying existing sessions.
    #[cfg(not(target_os = "macos"))]
    if matches!(err, keyring::Error::NoStorageAccess(_)) {
        return get_or_create_fallback_key(fallback_dir);
    }

    Err(err).context(
        "could not read the session encryption key from the OS keychain, and no \
         on-disk fallback key exists. Refusing to generate a new key because that \
         would make existing encrypted sessions permanently unrecoverable. Unlock / \
         grant keychain access and retry, or re-run setup to re-authenticate.",
    )
}

// Unused in the macOS release build (keychain-only there); used by dev, tests,
// and non-macOS release fallback.
#[allow(dead_code)]
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
