//! WhatsMiner legacy 4028 API (`get_token` + MD5 sign + AES-256-ECB) for miners that
//! block API v3 writes with the default password.

use crate::error::{ErrorCode, MinerPulseError};
use crate::tcp::TcpCgminerClient;
use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, BlockEncryptMut, KeyInit};
use aes::Aes256;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use ecb::{Decryptor, Encryptor};
use md5::compute as md5_hash;
use serde_json::Value;
use sha2::{Digest, Sha256};

type Aes256EcbEnc = Encryptor<Aes256>;
type Aes256EcbDec = Decryptor<Aes256>;

const ITOA64: &[u8] = b"./0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

struct LegacyToken {
    key: String,
    sign: String,
}

pub fn send_legacy_command(
    host: &str,
    port: u16,
    password: &str,
    cmd: &str,
) -> Result<Value, MinerPulseError> {
    let token = fetch_legacy_token(host, port, password)?;
    let api_cmd = cmd.replace("{sign}", &token.sign);
    let aes_key = legacy_aes_key(&token.key);
    let encrypted = aes_encrypt_ecb(&api_cmd, &aes_key)?;
    let packet = serde_json::json!({
        "enc": 1,
        "data": BASE64.encode(&encrypted),
    });
    let client = TcpCgminerClient::default();
    let raw = client
        .send_payload(host, port, &packet.to_string())
        .map_err(|_| MinerPulseError::conn_failed())?;
    let outer: Value = serde_json::from_str(&raw).map_err(|_| MinerPulseError::with_code(ErrorCode::ParseFailed))?;
    if let Some(data) = outer.get("enc").and_then(|v| v.as_str()) {
        if data.len() > 8 {
            let cipher = BASE64
                .decode(data)
                .map_err(|_| MinerPulseError::with_code(ErrorCode::ParseFailed))?;
        let plain = aes_decrypt_ecb(&cipher, &aes_key)?;
        let json_text = trim_json_suffix(&plain);
        return serde_json::from_str(json_text)
                .map_err(|_| MinerPulseError::with_code(ErrorCode::ParseFailed));
        }
    }
    if outer.get("enc").and_then(|v| v.as_i64()) == Some(1) {
        if let Some(data) = outer.get("data").and_then(|v| v.as_str()) {
            let cipher = BASE64
                .decode(data)
                .map_err(|_| MinerPulseError::with_code(ErrorCode::ParseFailed))?;
        let plain = aes_decrypt_ecb(&cipher, &aes_key)?;
        let json_text = trim_json_suffix(&plain);
        return serde_json::from_str(json_text)
                .map_err(|_| MinerPulseError::with_code(ErrorCode::ParseFailed));
        }
    }
    Ok(outer)
}

fn fetch_legacy_token(host: &str, port: u16, password: &str) -> Result<LegacyToken, MinerPulseError> {
    let client = TcpCgminerClient::default();
    let raw = client
        .send_payload(host, port, r#"{"cmd":"get_token"}"#)
        .map_err(|_| MinerPulseError::conn_failed())?;
    let value: Value = serde_json::from_str(&raw).map_err(|_| MinerPulseError::with_code(ErrorCode::ParseFailed))?;
    if value.get("Code").and_then(|c| c.as_i64()) != Some(134) {
        return Err(MinerPulseError::with_code(ErrorCode::ConnFailed));
    }
    let msg = value.get("Msg").ok_or_else(|| MinerPulseError::with_code(ErrorCode::ParseFailed))?;
    let salt = msg.get("salt").and_then(|v| v.as_str()).ok_or_else(|| MinerPulseError::with_code(ErrorCode::ParseFailed))?;
    let newsalt = msg
        .get("newsalt")
        .and_then(|v| v.as_str())
        .ok_or_else(|| MinerPulseError::with_code(ErrorCode::ParseFailed))?;
    let time = msg
        .get("time")
        .and_then(|v| v.as_str())
        .ok_or_else(|| MinerPulseError::with_code(ErrorCode::ParseFailed))?;
    let key = crypt_md5_hash(password, salt);
    let sign = crypt_md5_hash(&format!("{key}{time}"), newsalt);
    Ok(LegacyToken { key, sign })
}

fn legacy_aes_key(key: &str) -> [u8; 32] {
    let hex = Sha256::digest(key.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let mut out = [0u8; 32];
    for (i, chunk) in hex.as_bytes().chunks(2).enumerate().take(32) {
        let byte = u8::from_str_radix(std::str::from_utf8(chunk).unwrap_or("00"), 16).unwrap_or(0);
        out[i] = byte;
    }
    out
}

fn aes_encrypt_ecb(plain: &str, key: &[u8; 32]) -> Result<Vec<u8>, MinerPulseError> {
    let cipher = Aes256EcbEnc::new(key.into());
    let pos = plain.len();
    let mut buf = plain.as_bytes().to_vec();
    buf.resize(pos + 16, 0);
    cipher
        .encrypt_padded_mut::<Pkcs7>(&mut buf, pos)
        .map_err(|_| MinerPulseError::with_code(ErrorCode::ParseFailed))?;
    Ok(buf)
}

fn trim_json_suffix(plain: &str) -> &str {
    if let Some(end) = plain.rfind('}') {
        &plain[..=end]
    } else {
        plain.trim_end_matches('\0')
    }
}

fn aes_decrypt_ecb(cipher: &[u8], key: &[u8; 32]) -> Result<String, MinerPulseError> {
    let mut dec = Aes256EcbDec::new(key.into());
    let mut raw = cipher.to_vec();
    for chunk in raw.chunks_mut(16) {
        if chunk.len() == 16 {
            let block = aes::Block::from_mut_slice(chunk);
            dec.decrypt_block_mut(block);
        }
    }
    if let Some(&pad) = raw.last() {
        let pad = pad as usize;
        if pad > 0 && pad <= 16 && raw.len() >= pad {
            raw.truncate(raw.len() - pad);
        }
    }
    String::from_utf8(raw).map_err(|_| MinerPulseError::with_code(ErrorCode::ParseFailed))
}

fn crypt_md5_hash(password: &str, salt: &str) -> String {
    let full = crypt_md5(password, salt);
    full.rsplit('$').next().unwrap_or(&full).to_string()
}

/// Unix crypt-md5 (`$1$`) compatible with amazingFarm `MD5Crypt.crypt`.
fn crypt_md5(password: &str, salt: &str) -> String {
    let mut salt_body = salt;
    if let Some(stripped) = salt_body.strip_prefix("$1$") {
        salt_body = stripped;
    }
    if let Some((base, _)) = salt_body.rsplit_once('$') {
        salt_body = base;
    }
    let salt_body = &salt_body[..salt_body.len().min(8)];

    let password_bytes = password.as_bytes();
    let salt_bytes = salt_body.as_bytes();

    let mut ctx = Vec::new();
    ctx.extend_from_slice(password_bytes);
    ctx.extend_from_slice(b"$1$");
    ctx.extend_from_slice(salt_bytes);

    let final_pre = md5_hash([password_bytes, salt_bytes, password_bytes].concat()).0;

    let mut len = password.len();
    while len > 0 {
        let chunk = 16.min(len);
        ctx.extend_from_slice(&final_pre[..chunk]);
        len = len.saturating_sub(16);
    }

    let mut i = password.len();
    while i > 0 {
        if (i & 1) == 1 {
            ctx.push(0);
        } else {
            ctx.push(password_bytes[0]);
        }
        i >>= 1;
    }

    let mut final_hash = md5_hash(&ctx).0;

    for i in 0..1000 {
        let mut ctx1 = Vec::new();
        if (i & 1) == 1 {
            ctx1.extend_from_slice(password_bytes);
        } else {
            ctx1.extend_from_slice(&final_hash);
        }
        if i % 3 != 0 {
            ctx1.extend_from_slice(salt_bytes);
        }
        if i % 7 != 0 {
            ctx1.extend_from_slice(password_bytes);
        }
        if (i & 1) != 0 {
            ctx1.extend_from_slice(&final_hash);
        } else {
            ctx1.extend_from_slice(password_bytes);
        }
        final_hash = md5_hash(&ctx1).0;
    }

    let mut encoded = String::new();
    let mut value = ((final_hash[0] as u32) << 16)
        | ((final_hash[6] as u32) << 8)
        | final_hash[12] as u32;
    encoded.push_str(&to64(value, 4));
    value = ((final_hash[1] as u32) << 16)
        | ((final_hash[7] as u32) << 8)
        | final_hash[13] as u32;
    encoded.push_str(&to64(value, 4));
    value = ((final_hash[2] as u32) << 16)
        | ((final_hash[8] as u32) << 8)
        | final_hash[14] as u32;
    encoded.push_str(&to64(value, 4));
    value = ((final_hash[3] as u32) << 16)
        | ((final_hash[9] as u32) << 8)
        | final_hash[15] as u32;
    encoded.push_str(&to64(value, 4));
    value = ((final_hash[4] as u32) << 16) | ((final_hash[10] as u32) << 8) | final_hash[5] as u32;
    encoded.push_str(&to64(value, 4));
    value = final_hash[11] as u32;
    encoded.push_str(&to64(value, 2));

    format!("$1${salt_body}${encoded}")
}

fn to64(value: u32, count: usize) -> String {
    let mut out = String::with_capacity(count);
    let mut v = value;
    for _ in 0..count {
        out.push(ITOA64[(v & 0x3f) as usize] as char);
        v >>= 6;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "live miner on LAN"]
    fn live_legacy_power_roundtrip() {
        let host = "192.168.35.31";
        let pass = "admin";
        let off = send_legacy_command(
            host,
            4028,
            pass,
            r#"{"token":"{sign}","cmd":"power_off"}"#,
        )
        .expect("power_off");
        eprintln!("power_off: {off}");
        assert_eq!(off.get("STATUS").and_then(|v| v.as_str()), Some("S"));
        std::thread::sleep(std::time::Duration::from_secs(5));
        let on = send_legacy_command(
            host,
            4028,
            pass,
            r#"{"token":"{sign}","cmd":"power_on"}"#,
        )
        .expect("power_on");
        eprintln!("power_on: {on}");
        assert_eq!(on.get("STATUS").and_then(|v| v.as_str()), Some("S"));
    }
}
