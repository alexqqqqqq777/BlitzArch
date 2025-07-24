//! Cryptography module for handling encryption and decryption.

use aes_gcm::aead::{Aead, AeadInPlace};
use aes_gcm::{Aes256Gcm, Key, KeyInit, Nonce};
use pbkdf2::pbkdf2_hmac;
use rand::rngs::OsRng;
use rand::RngCore;
use sha2::Sha256;
// New KDF
use argon2::{Argon2, PasswordHasher, password_hash::{SaltString, PasswordHash, PasswordVerifier}};

const KEY_SIZE: usize = 32; // 256 bits for AES-256
const NONCE_SIZE: usize = 12; // 96 bits for GCM
const SALT_SIZE: usize = 16; // 128 bits for salt
const PBKDF2_ROUNDS: u32 = 100_000;
// Argon2 parameters (64 MiB, 3 passes) – aligns with OWASP recommendations
// Use lower memory in debug builds (including tests) to avoid OOM during CI
#[cfg(debug_assertions)]
const ARGON2_DEFAULT_MEM_KIB: u32 = 8192; // 8 MiB in debug/tests
#[cfg(not(debug_assertions))]
const ARGON2_DEFAULT_MEM_KIB: u32 = 65536; // 64 MiB in release
const ARGON2_ITER: u32 = 3;
const ARGON2_PARALLELISM: u32 = 1;

pub fn generate_salt() -> Vec<u8> {
    let mut salt = vec![0u8; SALT_SIZE];
    OsRng.fill_bytes(&mut salt);
    salt
}

pub fn derive_key_argon2(password: &str, salt: &[u8]) -> [u8; KEY_SIZE] {
    let mem_kib: u32 = std::env::var("BLITZ_ARGON2_MEM_KIB")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(ARGON2_DEFAULT_MEM_KIB);

    let params = argon2::Params::new(mem_kib, ARGON2_ITER, ARGON2_PARALLELISM, None)
        .expect("argon2 params");
    let argon2 = Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);
    let mut key = [0u8; KEY_SIZE];
    argon2.hash_password_into(password.as_bytes(), salt, &mut key).expect("argon2 hash");
    key
}

fn derive_key_pbkdf2(password: &str, salt: &[u8]) -> [u8; KEY_SIZE] {
    let mut key = [0u8; KEY_SIZE];
    pbkdf2_hmac::<Sha256>(password.as_bytes(), salt, PBKDF2_ROUNDS, &mut key);
    key
}

pub fn encrypt_prekey(
    data: &[u8],
    key_bytes: &[u8; KEY_SIZE],
) -> Result<(Vec<u8>, Vec<u8>), aes_gcm::Error> {
    let key = Key::<Aes256Gcm>::from_slice(key_bytes);
    let cipher = Aes256Gcm::new(key);
    let mut nonce_bytes = vec![0u8; NONCE_SIZE];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher.encrypt(nonce, data)?;
    Ok((ciphertext, nonce_bytes))
}

pub fn encrypt(
    data: &[u8],
    password: &str,
    salt: &[u8],
) -> Result<(Vec<u8>, Vec<u8>), aes_gcm::Error> {
    let key_bytes = derive_key_argon2(password, salt);
    let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
    let cipher = Aes256Gcm::new(key);

    let mut nonce_bytes = vec![0u8; NONCE_SIZE];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher.encrypt(nonce, data)?;
    Ok((ciphertext, nonce_bytes))
}

// --- In-place (zero-copy) helpers -----------------------------------------------------------
/// Encrypts `buf` in place with a pre-derived key. Appends 16-byte tag to the end of the same
/// buffer and returns the random 12-byte nonce.
pub fn encrypt_prekey_in_place(buf: &mut Vec<u8>, key_bytes: &[u8; KEY_SIZE]) -> Result<Vec<u8>, aes_gcm::Error> {
    use aes_gcm::aead::rand_core::RngCore;
    let key = Key::<Aes256Gcm>::from_slice(key_bytes);
    let cipher = Aes256Gcm::new(key);
    let mut nonce_bytes = [0u8; NONCE_SIZE];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    // Reserve space for tag at end
    let orig_len = buf.len();
    buf.resize(orig_len + 16, 0u8);
    let (data, tag_buf) = buf.split_at_mut(orig_len);
    let tag = cipher.encrypt_in_place_detached(nonce, b"", data)?; // AAD empty
    tag_buf.copy_from_slice(tag.as_slice());
    Ok(nonce_bytes.to_vec())
}

/// Decrypts `buf` in place (expects last 16 bytes to be AES-GCM tag). Shrinks buffer to
/// plaintext length on success.
pub fn decrypt_prekey_in_place(buf: &mut Vec<u8>, key_bytes: &[u8; KEY_SIZE], nonce: &[u8]) -> Result<(), aes_gcm::Error> {
    if buf.len() < 16 { return Err(aes_gcm::Error); }
    let tag_start = buf.len() - 16;
    let tag_bytes = buf[tag_start..].to_vec();
    buf.truncate(tag_start);

    let key = Key::<Aes256Gcm>::from_slice(key_bytes);
    let cipher = Aes256Gcm::new(key);
    let nonce = Nonce::from_slice(nonce);
    use aes_gcm::Tag;
    cipher.decrypt_in_place_detached(nonce, b"", buf, &Tag::from_slice(&tag_bytes))?;
    Ok(())
}

pub fn decrypt_prekey(
    ciphertext: &[u8],
    key_bytes: &[u8; KEY_SIZE],
    nonce: &[u8],
) -> Result<Vec<u8>, aes_gcm::Error> {
    let key = Key::<Aes256Gcm>::from_slice(key_bytes);
    let cipher = Aes256Gcm::new(key);
    let nonce = Nonce::from_slice(nonce);
    cipher.decrypt(nonce, ciphertext)
}

pub fn decrypt(
    ciphertext: &[u8],
    password: &str,
    salt: &[u8],
    nonce: &[u8],
) -> Result<Vec<u8>, aes_gcm::Error> {
    // Try Argon2id (new archives)
    let key_bytes = derive_key_argon2(password, salt);
    let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
    let cipher = Aes256Gcm::new(key);
    let nonce = Nonce::from_slice(nonce);
    if let Ok(pt) = cipher.decrypt(nonce, ciphertext) {
        return Ok(pt);
    }
    // Fallback to PBKDF2 for legacy archives
    let legacy_bytes = derive_key_pbkdf2(password, salt);
    let key_legacy = Key::<Aes256Gcm>::from_slice(&legacy_bytes);
    let cipher_legacy = Aes256Gcm::new(key_legacy);
    cipher_legacy.decrypt(nonce, ciphertext)
}
