//! Cryptography module for handling encryption and decryption.

use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, Key, KeyInit, Nonce};
use pbkdf2::pbkdf2_hmac;
use rand::rngs::OsRng;
use rand::RngCore;
use sha2::Sha256;

const KEY_SIZE: usize = 32; // 256 bits for AES-256
const NONCE_SIZE: usize = 12; // 96 bits for GCM
const SALT_SIZE: usize = 16; // 128 bits for salt
const PBKDF2_ROUNDS: u32 = 100_000;

pub fn generate_salt() -> Vec<u8> {
    let mut salt = vec![0u8; SALT_SIZE];
    OsRng.fill_bytes(&mut salt);
    salt
}

pub fn derive_key(password: &str, salt: &[u8]) -> [u8; KEY_SIZE] {
    let mut key = [0u8; KEY_SIZE];
    pbkdf2_hmac::<Sha256>(password.as_bytes(), salt, PBKDF2_ROUNDS, &mut key);
    key
}

pub fn encrypt(
    data: &[u8],
    password: &str,
    salt: &[u8],
) -> Result<(Vec<u8>, Vec<u8>), aes_gcm::Error> {
    let key_bytes = derive_key(password, salt);
    let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
    let cipher = Aes256Gcm::new(key);

    let mut nonce_bytes = vec![0u8; NONCE_SIZE];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher.encrypt(nonce, data)?;
    Ok((ciphertext, nonce_bytes))
}

pub fn decrypt(
    ciphertext: &[u8],
    password: &str,
    salt: &[u8],
    nonce: &[u8],
) -> Result<Vec<u8>, aes_gcm::Error> {
    let key_bytes = derive_key(password, salt);
    let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
    let cipher = Aes256Gcm::new(key);
    let nonce = Nonce::from_slice(nonce);

    cipher.decrypt(nonce, ciphertext)
}
