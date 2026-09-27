use keyring::Entry;

use crate::error::AppError;

const SERVICE: &str = "XUnFollow";
const ACCOUNT: &str = "twitterapi.io";

fn entry() -> Result<Entry, AppError> {
    Entry::new(SERVICE, ACCOUNT).map_err(|error| AppError::SecureStore(error.to_string()))
}

pub fn save_api_key(api_key: &str) -> Result<(), AppError> {
    let key = api_key.trim();
    if key.is_empty() || key.len() > 1024 {
        return Err(AppError::Validation("API Key 无效".into()));
    }
    let item = entry()?;
    item
        .set_password(key)
        .map_err(|error| AppError::SecureStore(error.to_string()))?;

    // Confirm the OS credential store accepted the write before making a
    // provider request. This keeps a Keychain/Windows Credential Manager
    // problem distinct from an invalid provider key, without ever logging it.
    let saved = item
        .get_password()
        .map_err(|error| AppError::SecureStore(error.to_string()))?;
    if saved != key {
        return Err(AppError::SecureStore("系统安全存储未确认写入 API Key".into()));
    }
    Ok(())
}

pub fn load_api_key() -> Result<String, AppError> {
    entry()?
        .get_password()
        .map_err(|_| AppError::Validation("还没有保存 TwitterAPI.io API Key".into()))
}

pub fn has_api_key() -> bool {
    entry()
        .and_then(|item| {
            item.get_password()
                .map_err(|error| AppError::SecureStore(error.to_string()))
        })
        .is_ok()
}
