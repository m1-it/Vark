use base64::{engine::general_purpose::STANDARD, Engine as _};
use ring::{
    aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_256_GCM},
    digest::{digest, SHA256},
    rand::{SecureRandom, SystemRandom},
};
use std::{
    env,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};
use thiserror::Error;

const ENCRYPTED_PREFIX: &str = "vark1:";
const NONCE_LENGTH: usize = 12;

#[derive(Debug, Error)]
pub(crate) enum StorageCryptoError {
    #[error("VARK_STORAGE_KEY wajib tersedia untuk membaca data terenkripsi")]
    KeyRequired,
    #[error("VARK_STORAGE_KEY harus memiliki setidaknya 32 byte")]
    WeakKey,
    #[error("VARK_STORAGE_KEY bukan teks UTF-8 yang valid")]
    InvalidKey,
    #[error("gagal mengenkripsi data storage")]
    Encryption,
    #[error("record terenkripsi rusak atau autentikasinya gagal")]
    Authentication,
}

pub(crate) fn encryption_configured() -> bool {
    env::var_os("VARK_STORAGE_KEY").is_some()
}

pub(crate) fn is_encrypted_line(line: &str) -> bool {
    line.starts_with(ENCRYPTED_PREFIX)
}

fn derive_key(secret: &[u8]) -> Result<[u8; 32], StorageCryptoError> {
    if secret.len() < 32 {
        return Err(StorageCryptoError::WeakKey);
    }
    let hashed = digest(&SHA256, secret);
    let mut key = [0u8; 32];
    key.copy_from_slice(hashed.as_ref());
    Ok(key)
}

fn configured_key() -> Result<Option<[u8; 32]>, StorageCryptoError> {
    match env::var("VARK_STORAGE_KEY") {
        Ok(secret) => derive_key(secret.as_bytes()).map(Some),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(env::VarError::NotUnicode(_)) => Err(StorageCryptoError::InvalidKey),
    }
}

pub(crate) fn seal_line(plaintext: &[u8]) -> Result<String, StorageCryptoError> {
    match configured_key()? {
        Some(key) => seal_with_key(plaintext, &key),
        None => String::from_utf8(plaintext.to_vec()).map_err(|_| StorageCryptoError::Encryption),
    }
}

pub(crate) fn open_line(line: &str) -> Result<Vec<u8>, StorageCryptoError> {
    if let Some(encoded) = line.strip_prefix(ENCRYPTED_PREFIX) {
        let key = configured_key()?.ok_or(StorageCryptoError::KeyRequired)?;
        open_with_key(encoded, &key)
    } else {
        Ok(line.as_bytes().to_vec())
    }
}

fn seal_with_key(plaintext: &[u8], key_bytes: &[u8; 32]) -> Result<String, StorageCryptoError> {
    let key = LessSafeKey::new(
        UnboundKey::new(&AES_256_GCM, key_bytes).map_err(|_| StorageCryptoError::Encryption)?,
    );
    let mut nonce_bytes = [0u8; NONCE_LENGTH];
    SystemRandom::new()
        .fill(&mut nonce_bytes)
        .map_err(|_| StorageCryptoError::Encryption)?;
    let mut ciphertext = plaintext.to_vec();
    key.seal_in_place_append_tag(
        Nonce::assume_unique_for_key(nonce_bytes),
        Aad::empty(),
        &mut ciphertext,
    )
    .map_err(|_| StorageCryptoError::Encryption)?;
    let mut payload = Vec::with_capacity(NONCE_LENGTH + ciphertext.len());
    payload.extend_from_slice(&nonce_bytes);
    payload.extend_from_slice(&ciphertext);
    Ok(format!("{ENCRYPTED_PREFIX}{}", STANDARD.encode(payload)))
}

fn open_with_key(encoded: &str, key_bytes: &[u8; 32]) -> Result<Vec<u8>, StorageCryptoError> {
    let mut payload = STANDARD
        .decode(encoded)
        .map_err(|_| StorageCryptoError::Authentication)?;
    if payload.len() <= NONCE_LENGTH {
        return Err(StorageCryptoError::Authentication);
    }
    let mut nonce_bytes = [0u8; NONCE_LENGTH];
    nonce_bytes.copy_from_slice(&payload[..NONCE_LENGTH]);
    let key = LessSafeKey::new(
        UnboundKey::new(&AES_256_GCM, key_bytes).map_err(|_| StorageCryptoError::Encryption)?,
    );
    let plaintext = key
        .open_in_place(
            Nonce::assume_unique_for_key(nonce_bytes),
            Aad::empty(),
            &mut payload[NONCE_LENGTH..],
        )
        .map_err(|_| StorageCryptoError::Authentication)?;
    Ok(plaintext.to_vec())
}

pub(crate) fn lock_path(path: &Path, exclusive: bool) -> io::Result<File> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        ensure_private_directory(parent)?;
    }
    let mut lock_name = path.as_os_str().to_os_string();
    lock_name.push(".lock");
    let lock_path = PathBuf::from(lock_name);
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&lock_path)?;
    protect_file(&lock_path)?;
    if exclusive {
        lock.lock()?;
    } else {
        lock.lock_shared()?;
    }
    Ok(lock)
}

pub(crate) fn ensure_private_directory(path: &Path) -> io::Result<()> {
    let mut missing = Vec::new();
    let mut current = path;
    while !current.exists() {
        missing.push(current.to_path_buf());
        let Some(parent) = current.parent() else {
            break;
        };
        if parent == current {
            break;
        }
        current = parent;
    }
    fs::create_dir_all(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for directory in missing.iter().rev() {
            fs::set_permissions(directory, fs::Permissions::from_mode(0o700))?;
        }
    }
    #[cfg(not(unix))]
    let _ = missing;
    Ok(())
}

pub(crate) fn protect_file(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

pub(crate) fn private_append_file(path: &Path) -> io::Result<File> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        ensure_private_directory(parent)?;
    }
    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .append(true)
        .open(path)?;
    protect_file(path)?;
    Ok(file)
}

pub(crate) fn write_private_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        ensure_private_directory(parent)?;
    }
    let mut temporary_name = path.as_os_str().to_os_string();
    temporary_name.push(format!(".tmp-{}", std::process::id()));
    let temporary_path = PathBuf::from(temporary_name);
    let mut options = OpenOptions::new();
    options.create(true).truncate(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temporary_path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    protect_file(&temporary_path)?;
    fs::rename(&temporary_path, path)?;
    protect_file(path)?;
    Ok(())
}

pub(crate) fn redact(input: &str) -> String {
    let mut redact_next = false;
    let mut output = Vec::new();
    for word in input.split_whitespace() {
        if redact_next {
            output.push("[REDACTED]".to_owned());
            redact_next = false;
            continue;
        }
        let lower = word.to_ascii_lowercase();
        let label = lower
            .trim_matches(|character: char| !character.is_ascii_alphanumeric() && character != '_');
        let digits = word.chars().filter(char::is_ascii_digit).count();
        let inline_secret = [
            "api_key=",
            "api-key=",
            "password=",
            "passwd=",
            "secret=",
            "token=",
            "password:",
            "passwd:",
            "secret:",
            "token:",
            "api_key:",
            "api-key:",
        ]
        .iter()
        .any(|marker| lower.starts_with(marker) && lower.len() > marker.len());
        let secret_label = [
            "password",
            "passwd",
            "secret",
            "api_key",
            "api-key",
            "token",
            "authorization",
        ]
        .contains(&label);
        let bearer_label = lower == "bearer";
        if word.contains('@') || digits >= 7 || inline_secret || lower.contains("bearer") {
            output.push("[REDACTED]".to_owned());
            redact_next = bearer_label;
        } else if secret_label {
            output.push(word.to_owned());
            redact_next = true;
        } else {
            output.push(word.to_owned());
        }
    }
    output.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authenticated_encryption_hides_and_authenticates_plaintext() {
        let key = derive_key(b"a sufficiently long storage key for tests").unwrap();
        let sealed = seal_with_key(b"private conversation text", &key).unwrap();
        assert!(!sealed.contains("private conversation text"));
        assert_eq!(
            open_with_key(sealed.strip_prefix(ENCRYPTED_PREFIX).unwrap(), &key).unwrap(),
            b"private conversation text"
        );
        let wrong_key = derive_key(b"a different long storage key for tests").unwrap();
        assert!(open_with_key(sealed.strip_prefix(ENCRYPTED_PREFIX).unwrap(), &wrong_key).is_err());
    }

    #[test]
    fn rejects_short_storage_keys() {
        assert!(matches!(
            derive_key(b"too short"),
            Err(StorageCryptoError::WeakKey)
        ));
    }

    #[test]
    fn redaction_covers_separate_and_inline_credentials() {
        let sanitized =
            redact("Bearer bearer-secret password: rahasia token=token-secret api_key: key-secret");
        assert!(!sanitized.contains("bearer-secret"));
        assert!(!sanitized.contains("rahasia"));
        assert!(!sanitized.contains("token-secret"));
        assert!(!sanitized.contains("key-secret"));
    }
}
