//! AES helpers for WhatsMiner API v3 encrypted `param` fields.

use crate::error::{ErrorCode, MinerPulseError};
use aes::cipher::{block_padding::Pkcs7, BlockEncryptMut, KeyInit};
use aes::Aes256;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use ecb::Encryptor;
use sha2::{Digest, Sha256};

type Aes256EcbEnc = Encryptor<Aes256>;

/// API v3: full SHA256(`cmd` + `password` + `salt` + `ts`) as AES-256 key.
pub fn v3_aes_key(cmd: &str, password: &str, salt: &str, ts: i64) -> [u8; 32] {
    let input = format!("{cmd}{password}{salt}{ts}");
    Sha256::digest(input.as_bytes()).into()
}

pub fn aes_encrypt_ecb_pkcs7(plain: &[u8], key: &[u8; 32]) -> Result<Vec<u8>, MinerPulseError> {
    let cipher = Aes256EcbEnc::new(key.into());
    let pos = plain.len();
    let mut buf = plain.to_vec();
    buf.resize(pos + 16, 0);
    cipher
        .encrypt_padded_mut::<Pkcs7>(&mut buf, pos)
        .map_err(|_| MinerPulseError::with_code(ErrorCode::ParseFailed))?;
    Ok(buf)
}

pub fn encrypt_v3_param(
    cmd: &str,
    password: &str,
    salt: &str,
    ts: i64,
    plain_json: &str,
) -> Result<String, MinerPulseError> {
    let key = v3_aes_key(cmd, password, salt, ts);
    let encrypted = aes_encrypt_ecb_pkcs7(plain_json.as_bytes(), &key)?;
    Ok(BASE64.encode(encrypted))
}
