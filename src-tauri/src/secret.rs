//! 本机敏感文本的加密存储（Windows DPAPI，仅当前用户可解密）。
//!
//! 用于远程机器密码与 DeepSeek API Key。存储格式：
//! - `dpapi:<hex>`：Windows DPAPI 加密；
//! - `plain:<text>`：DPAPI 不可用时的退路 / 非 Windows 平台。
//!
//! 非 Windows 平台没有 DPAPI，只能明文保存；应用仍可正常构建 `wmfold-cli`
//! 所依赖的共享核心（本模块只在桌面端使用）。

#[cfg(windows)]
fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

#[cfg(windows)]
fn hex_decode(text: &str) -> Option<Vec<u8>> {
    if text.len() % 2 != 0 {
        return None;
    }
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len() / 2);
    let mut i = 0;
    while i < bytes.len() {
        let hi = (bytes[i] as char).to_digit(16)?;
        let lo = (bytes[i + 1] as char).to_digit(16)?;
        out.push(((hi << 4) | lo) as u8);
        i += 2;
    }
    Some(out)
}

/// 加密敏感文本（空串保持为空）。
#[cfg(windows)]
pub fn protect(plain: &str) -> String {
    use std::ptr;
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{
        CryptProtectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };

    if plain.is_empty() {
        return String::new();
    }
    unsafe {
        let mut bytes = plain.as_bytes().to_vec();
        let input = CRYPT_INTEGER_BLOB {
            cbData: bytes.len() as u32,
            pbData: bytes.as_mut_ptr(),
        };
        let mut output: CRYPT_INTEGER_BLOB = std::mem::zeroed();
        let ok = CryptProtectData(
            &input,
            ptr::null(),
            ptr::null(),
            ptr::null(),
            ptr::null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        );
        if ok == 0 || output.pbData.is_null() {
            // DPAPI 失败时退回明文（不应发生，但不要因此丢失密码 / Key）
            return format!("plain:{plain}");
        }
        let data = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        let _ = LocalFree(output.pbData as _);
        format!("dpapi:{}", hex_encode(&data))
    }
}

/// 解密敏感文本；无法解密时返回空串。
#[cfg(windows)]
pub fn unprotect(stored: &str) -> String {
    use std::ptr;
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{
        CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };

    let Some(hex) = stored.strip_prefix("dpapi:") else {
        return stored.strip_prefix("plain:").unwrap_or(stored).to_string();
    };
    let Some(mut data) = hex_decode(hex) else {
        return String::new();
    };
    unsafe {
        let input = CRYPT_INTEGER_BLOB {
            cbData: data.len() as u32,
            pbData: data.as_mut_ptr(),
        };
        let mut output: CRYPT_INTEGER_BLOB = std::mem::zeroed();
        let ok = CryptUnprotectData(
            &input,
            ptr::null_mut(),
            ptr::null(),
            ptr::null(),
            ptr::null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        );
        if ok == 0 || output.pbData.is_null() {
            return String::new();
        }
        let bytes = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        let _ = LocalFree(output.pbData as _);
        String::from_utf8_lossy(&bytes).to_string()
    }
}

#[cfg(not(windows))]
pub fn protect(plain: &str) -> String {
    format!("plain:{plain}")
}

#[cfg(not(windows))]
pub fn unprotect(stored: &str) -> String {
    stored.strip_prefix("plain:").unwrap_or(stored).to_string()
}

/// 只显示末尾若干位，用于界面提示（不泄露完整密钥）。
pub fn preview(secret: &str) -> String {
    let count = secret.chars().count();
    if count <= 4 {
        return "*".repeat(count);
    }
    let tail: String = secret.chars().skip(count - 4).collect();
    format!("…{tail}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_keeps_text() {
        let stored = protect("sk-test-123456");
        let back = unprotect(&stored);
        assert_eq!(back, "sk-test-123456");
    }

    #[test]
    fn empty_stays_empty() {
        assert_eq!(protect(""), "");
        assert_eq!(unprotect(""), "");
    }

    #[test]
    fn preview_masks_prefix() {
        assert_eq!(preview("sk-1234567890"), "…7890");
        assert_eq!(preview("abc"), "***");
    }
}
