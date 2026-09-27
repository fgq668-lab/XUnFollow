mod db;
mod error;
mod models;
mod provider;

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use db::AppDb;
use error::AppError;
use models::{Bootstrap, CostEstimate, ImportedDecision};
use provider::TwitterApiIo;
use std::collections::BTreeMap;
use tauri::{Manager, State};
use tauri_plugin_opener::OpenerExt;
use url::Url;

#[derive(Clone)]
struct AppState {
    db: Arc<AppDb>,
    sync_running: Arc<AtomicBool>,
}

#[tauri::command]
fn bootstrap(state: State<'_, AppState>) -> Result<Bootstrap, AppError> {
    let mut snapshot = state.db.bootstrap()?;
    snapshot.sync_running = state.sync_running.load(Ordering::Acquire);
    Ok(snapshot)
}

#[tauri::command]
fn save_api_key(state: State<'_, AppState>, api_key: String) -> Result<(), AppError> {
    state.db.save_api_key(&api_key)
}

#[tauri::command]
async fn estimate_cost(
    state: State<'_, AppState>,
    handle: String,
    hard_cap_usd: String,
) -> Result<CostEstimate, AppError> {
    let cap = parse_usd_to_micros(&hard_cap_usd)?;
    let provider = TwitterApiIo::new(state.db.load_api_key()?)?;
    provider.estimate(&normalise_handle(&handle)?, cap).await
}

#[tauri::command]
fn start_scan(
    state: State<'_, AppState>,
    handle: String,
    hard_cap_usd: String,
) -> Result<(), AppError> {
    let cap = parse_usd_to_micros(&hard_cap_usd)?;
    let provider = TwitterApiIo::new(state.db.load_api_key()?)?;
    launch_scan(&state, provider, normalise_handle(&handle)?, cap, false)
}

#[tauri::command]
fn resume_scan_once(
    state: State<'_, AppState>,
    handle: String,
    hard_cap_usd: String,
    acknowledge_ambiguous_retry: bool,
) -> Result<(), AppError> {
    if !acknowledge_ambiguous_retry {
        return Err(AppError::Validation("必须明确确认一次保守重试".into()));
    }
    let cap = parse_usd_to_micros(&hard_cap_usd)?;
    let provider = TwitterApiIo::new(state.db.load_api_key()?)?;
    launch_scan(&state, provider, normalise_handle(&handle)?, cap, true)
}

fn launch_scan(
    state: &AppState,
    provider: TwitterApiIo,
    handle: String,
    hard_cap_usd: i64,
    acknowledge_ambiguous_retry: bool,
) -> Result<(), AppError> {
    TwitterApiIo::prepare_scan(&state.db, &handle, hard_cap_usd)?;
    if state.sync_running.swap(true, Ordering::AcqRel) {
        return Err(AppError::Validation("同步已在后台进行中".into()));
    }
    let db = state.db.clone();
    let running = state.sync_running.clone();
    tauri::async_runtime::spawn(async move {
        let _ = provider
            .scan(&db, &handle, hard_cap_usd, acknowledge_ambiguous_retry)
            .await;
        running.store(false, Ordering::Release);
    });
    Ok(())
}

#[tauri::command]
fn record_decision(
    state: State<'_, AppState>,
    stable_x_id: String,
    status: String,
) -> Result<(), AppError> {
    state.db.record_decision(&stable_x_id, &status)
}

#[tauri::command]
fn undo_last(state: State<'_, AppState>) -> Result<(), AppError> {
    state.db.undo_last()
}

#[tauri::command]
fn continue_batch(state: State<'_, AppState>) -> Result<i64, AppError> {
    state.db.continue_batch()
}

#[tauri::command]
fn import_decisions(
    state: State<'_, AppState>,
    decisions: BTreeMap<String, ImportedDecision>,
) -> Result<i64, AppError> {
    state.db.import_decisions(decisions)
}

#[tauri::command]
fn open_external_profile(app: tauri::AppHandle, url: String) -> Result<(), AppError> {
    let parsed = validated_external_profile_url(&url)?;
    app.opener()
        .open_url(parsed.as_str(), None::<&str>)
        .map_err(|error| AppError::Network(error.to_string()))
}

fn validated_external_profile_url(url: &str) -> Result<Url, AppError> {
    let parsed = Url::parse(url).map_err(|_| AppError::ExternalUrlRejected)?;
    let host = parsed.host_str().unwrap_or_default();
    if parsed.scheme() != "https"
        || !matches!(host, "x.com" | "www.x.com")
        || parsed.username() != ""
        || parsed.password().is_some()
    {
        return Err(AppError::ExternalUrlRejected);
    }
    Ok(parsed)
}

fn normalise_handle(value: &str) -> Result<String, AppError> {
    let handle = value.trim().trim_start_matches('@').to_ascii_lowercase();
    if handle.is_empty()
        || handle.len() > 15
        || !handle
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        return Err(AppError::Validation("X 用户名无效".into()));
    }
    Ok(handle)
}

fn parse_usd_to_micros(value: &str) -> Result<i64, AppError> {
    let value = value.trim();
    let (whole, fraction) = value.split_once('.').unwrap_or((value, ""));
    if whole.is_empty()
        || !whole.bytes().all(|byte| byte.is_ascii_digit())
        || fraction.len() > 6
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(AppError::Validation(
            "费用上限必须是最多六位小数的美元金额".into(),
        ));
    }
    let whole: i64 = whole
        .parse()
        .map_err(|_| AppError::Validation("费用上限无效".into()))?;
    let fraction = format!("{fraction:0<6}")
        .parse::<i64>()
        .map_err(|_| AppError::Validation("费用上限无效".into()))?;
    let micros = whole
        .checked_mul(1_000_000)
        .and_then(|item| item.checked_add(fraction))
        .ok_or_else(|| AppError::Validation("费用上限过大".into()))?;
    if !(10_000..=1_000_000_000).contains(&micros) {
        return Err(AppError::Validation(
            "费用上限必须在 $0.01 到 $1000 之间".into(),
        ));
    }
    Ok(micros)
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let path = app.path().app_data_dir()?.join("xunfollow.sqlite3");
            app.manage(AppState {
                db: Arc::new(AppDb::new(path)?),
                sync_running: Arc::new(AtomicBool::new(false)),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            bootstrap,
            save_api_key,
            estimate_cost,
            start_scan,
            resume_scan_once,
            record_decision,
            undo_last,
            continue_batch,
            import_decisions,
            open_external_profile,
        ])
        .run(tauri::generate_context!())
        .expect("error while running XUnFollow");
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_micro_dollar_caps_without_float_rounding() {
        assert_eq!(parse_usd_to_micros("0.154134").unwrap(), 154_134);
        assert!(parse_usd_to_micros("0.0001").is_err());
        assert!(parse_usd_to_micros("1.1234567").is_err());
    }

    #[test]
    fn only_allows_known_https_x_hosts() {
        assert!(validated_external_profile_url("https://x.com/example").is_ok());
        assert!(validated_external_profile_url("https://www.x.com/example").is_ok());
        assert!(validated_external_profile_url("http://x.com/example").is_err());
        assert!(validated_external_profile_url("https://x.com.evil.example/example").is_err());
        assert!(validated_external_profile_url("https://user@x.com/example").is_err());
    }
}
