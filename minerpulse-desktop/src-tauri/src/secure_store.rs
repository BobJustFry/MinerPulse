//! At-rest encryption for local credential storage.
//!
//! Windows: DPAPI (`CryptProtectData`/`CryptUnprotectData`) bound to the current
//! user account — no key material stored on disk. Other platforms: passthrough
//! (desktop target is Windows; keeps the code compiling/testable elsewhere).

/// Magic header identifying an encrypted blob written by this app.
const MAGIC: &[u8; 4] = b"MPE1";

pub fn is_encrypted(bytes: &[u8]) -> bool {
    bytes.len() >= MAGIC.len() && &bytes[..MAGIC.len()] == MAGIC
}

/// Encrypt plaintext for on-disk storage. Returns a self-describing blob.
pub fn encrypt(plaintext: &[u8]) -> Result<Vec<u8>, String> {
    let cipher = platform_encrypt(plaintext)?;
    let mut out = Vec::with_capacity(MAGIC.len() + cipher.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&cipher);
    Ok(out)
}

/// Decrypt a blob previously produced by [`encrypt`].
pub fn decrypt(blob: &[u8]) -> Result<Vec<u8>, String> {
    if !is_encrypted(blob) {
        return Err("not_encrypted".to_string());
    }
    platform_decrypt(&blob[MAGIC.len()..])
}

#[cfg(windows)]
fn platform_encrypt(plaintext: &[u8]) -> Result<Vec<u8>, String> {
    use windows::Win32::Security::Cryptography::{CryptProtectData, CRYPT_INTEGER_BLOB};

    unsafe {
        let mut input = CRYPT_INTEGER_BLOB {
            cbData: plaintext.len() as u32,
            pbData: plaintext.as_ptr() as *mut u8,
        };
        let mut output = CRYPT_INTEGER_BLOB::default();
        CryptProtectData(
            &mut input,
            None,
            None,
            None,
            None,
            0,
            &mut output,
        )
        .map_err(|e| format!("dpapi_encrypt: {e}"))?;
        Ok(read_and_free_blob(&mut output))
    }
}

#[cfg(windows)]
fn platform_decrypt(cipher: &[u8]) -> Result<Vec<u8>, String> {
    use windows::Win32::Security::Cryptography::{CryptUnprotectData, CRYPT_INTEGER_BLOB};

    unsafe {
        let mut input = CRYPT_INTEGER_BLOB {
            cbData: cipher.len() as u32,
            pbData: cipher.as_ptr() as *mut u8,
        };
        let mut output = CRYPT_INTEGER_BLOB::default();
        CryptUnprotectData(
            &mut input,
            None,
            None,
            None,
            None,
            0,
            &mut output,
        )
        .map_err(|e| format!("dpapi_decrypt: {e}"))?;
        Ok(read_and_free_blob(&mut output))
    }
}

#[cfg(windows)]
unsafe fn read_and_free_blob(
    blob: &mut windows::Win32::Security::Cryptography::CRYPT_INTEGER_BLOB,
) -> Vec<u8> {
    use windows::Win32::Foundation::{LocalFree, HLOCAL};

    let slice = std::slice::from_raw_parts(blob.pbData, blob.cbData as usize);
    let out = slice.to_vec();
    if !blob.pbData.is_null() {
        let _ = LocalFree(HLOCAL(blob.pbData as *mut core::ffi::c_void));
    }
    out
}

#[cfg(not(windows))]
fn platform_encrypt(plaintext: &[u8]) -> Result<Vec<u8>, String> {
    Ok(plaintext.to_vec())
}

#[cfg(not(windows))]
fn platform_decrypt(cipher: &[u8]) -> Result<Vec<u8>, String> {
    Ok(cipher.to_vec())
}

#[cfg(all(test, not(windows)))]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_passthrough() {
        let data = b"{\"hello\":\"world\"}";
        let enc = encrypt(data).unwrap();
        assert!(is_encrypted(&enc));
        assert_eq!(decrypt(&enc).unwrap(), data);
    }
}
