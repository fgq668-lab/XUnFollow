mod creator;
mod creator_reference;
mod creator_text;
mod db;
mod error;
mod models;
mod network;
mod preferences;
mod provider;
mod workbench;

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use db::AppDb;
use error::AppError;
use models::{Bootstrap, CostEstimate, ImportedDecision};
use preferences::Preferences;

#[tauri::command]
fn save_workbench_settings(
    state: State<'_, AppState>,
    preferences: Preferences,
    persona: Persona,
    twitter_key: String,
    deepseek_key: String,
) -> Result<(), AppError> {
    state
        .db
        .save_workbench_settings(&preferences, &persona, &twitter_key, &deepseek_key)
}
use creator::{CreatorConfig, CreatorSnapshot};
use provider::TwitterApiIo;
use std::collections::BTreeMap;
use tauri::{Manager, State};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_opener::OpenerExt;
use url::Url;
use workbench::{Persona, ReplyConfig, WorkbenchSnapshot};

fn launch_creator_worker(
    state: &AppState,
    prepare: impl FnOnce(&AppDb) -> Result<(), AppError>,
) -> Result<(), AppError> {
    if state.creator_running.swap(true, Ordering::AcqRel) {
        return Err(AppError::Validation("创作任务正在运行".into()));
    }
    if let Err(error) = prepare(&state.db) {
        state.creator_running.store(false, Ordering::Release);
        return Err(error);
    }
    let db = state.db.clone();
    let running = state.creator_running.clone();
    state.creator_continue.store(true, Ordering::Release);
    let keep = state.creator_continue.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = creator::run_creator(&db, &keep).await {
            creator::pause_error(&db, &e);
        }
        running.store(false, Ordering::Release);
    });
    Ok(())
}
fn creator_idle(state: &AppState) -> Result<(), AppError> {
    if state.creator_running.load(Ordering::Acquire) {
        Err(AppError::Validation("请先暂停并等待当前请求完成".into()))
    } else {
        Ok(())
    }
}
#[tauri::command]
fn creator_snapshot(state: State<'_, AppState>) -> Result<CreatorSnapshot, AppError> {
    state
        .db
        .creator_snapshot(state.creator_running.load(Ordering::Acquire))
}
#[tauri::command]
fn save_creator_goal(state: State<'_, AppState>, target: i64) -> Result<(), AppError> {
    state.db.save_creator_goal(target)
}
#[tauri::command]
fn start_creator_run(state: State<'_, AppState>, config: CreatorConfig) -> Result<(), AppError> {
    launch_creator_worker(&state, |db| db.create_creator_run(&config).map(|_| ()))
}
#[tauri::command]
fn restart_creator_run(
    state: State<'_, AppState>,
    config: CreatorConfig,
    expected_ids: Vec<i64>,
) -> Result<(), AppError> {
    launch_creator_worker(&state, |db| {
        db.restart_creator_run(&config, &expected_ids).map(|_| ())
    })
}
#[tauri::command]
fn creator_reference_snapshot(
    state: State<'_, AppState>,
) -> Result<creator_reference::ReferenceSnapshot, AppError> {
    state.db.reference_snapshot()
}
#[tauri::command]
fn save_creator_reference_selection(
    state: State<'_, AppState>,
    handles: Vec<String>,
) -> Result<(), AppError> {
    state.db.save_reference_selection(&handles)
}
#[tauri::command]
async fn read_creator_reference(
    state: State<'_, AppState>,
    username: String,
    cap: String,
    acknowledge_uncertain_cost: bool,
) -> Result<creator_reference::BloggerReference, AppError> {
    creator_reference::read(&state.db, &username, &cap, acknowledge_uncertain_cost).await
}
#[tauri::command]
fn save_creator_reference_guide(
    state: State<'_, AppState>,
    username: String,
    guide: String,
) -> Result<(), AppError> {
    state.db.save_reference_guide(&username, &guide)
}
#[tauri::command]
fn rewrite_creator_draft(
    state: State<'_, AppState>,
    id: i64,
    ai_cap_usd: String,
) -> Result<(), AppError> {
    launch_creator_worker(&state, |db| {
        db.create_rewrite_run(id, &ai_cap_usd).map(|_| ())
    })
}
#[tauri::command]
fn undo_creator_rewrite(state: State<'_, AppState>, id: i64) -> Result<(), AppError> {
    creator_idle(&state)?;
    state.db.undo_creator_rewrite(id)
}
#[tauri::command]
fn start_screenshot_run(
    state: State<'_, AppState>,
    image_data: String,
    notes: String,
    origin_url: String,
    count: i64,
    ai_cap_usd: String,
) -> Result<(), AppError> {
    launch_creator_worker(&state, |db| {
        db.create_screenshot_run(&image_data, &notes, &origin_url, count, &ai_cap_usd)
            .map(|_| ())
    })
}
#[tauri::command]
fn resume_creator_run(
    state: State<'_, AppState>,
    acknowledge_uncertain_cost: bool,
) -> Result<(), AppError> {
    launch_creator_worker(&state, |db| {
        db.resume_creator(acknowledge_uncertain_cost).map(|_| ())
    })
}
#[tauri::command]
fn stop_creator_run(state: State<'_, AppState>) {
    state.creator_continue.store(false, Ordering::Release);
}
#[tauri::command]
fn end_creator_run(state: State<'_, AppState>) -> Result<(), AppError> {
    creator_idle(&state)?;
    state.db.end_creator()
}
#[tauri::command]
fn raise_creator_cap(
    state: State<'_, AppState>,
    kind: String,
    cap: String,
) -> Result<(), AppError> {
    creator_idle(&state)?;
    state.db.raise_creator_cap(&kind, &cap)
}
#[tauri::command]
fn edit_creator_draft(
    state: State<'_, AppState>,
    id: i64,
    title: String,
    body: String,
    scheduled_at: Option<String>,
) -> Result<(), AppError> {
    state
        .db
        .edit_creator(id, &title, &body, scheduled_at.as_deref())
}
#[tauri::command]
fn set_creator_status(state: State<'_, AppState>, id: i64, status: String) -> Result<(), AppError> {
    state.db.set_creator_status(id, &status)
}
#[tauri::command]
fn approve_creator_run(state: State<'_, AppState>) -> Result<i64, AppError> {
    creator_idle(&state)?;
    state.db.approve_creator_run()
}
#[tauri::command]
fn plan_creator(state: State<'_, AppState>, replan_all: bool) -> Result<(), AppError> {
    creator_idle(&state)?;
    state.db.plan_creator(replan_all)
}
#[tauri::command]
fn snooze_creator(state: State<'_, AppState>, id: i64) -> Result<(), AppError> {
    state.db.snooze_creator(id)
}
#[tauri::command]
fn creator_calendar(state: State<'_, AppState>) -> Result<String, AppError> {
    state.db.creator_calendar()
}
#[tauri::command]
fn copy_creator_draft(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    id: i64,
    include_sources: bool,
) -> Result<(), AppError> {
    app.clipboard()
        .write_text(state.db.creator_copy_text(id, include_sources)?)
        .map_err(|_| AppError::Validation("复制失败，请重试".into()))
}
#[tauri::command]
fn copy_support_address(app: tauri::AppHandle, chain: String) -> Result<(), AppError> {
    let address = match chain.as_str() {
        "BSC" => "0xD14fd8765c21c2b97539BEE00189f8118AAdbe52",
        "SOL" => "CmNkNVkobbHAyn3jsaEBaJoAcaNbrTyo7vUwSaSmrVme",
        "DOGE" => "DQUXapWhCKUbiNX7YJkeTNE1q3zCQAzriV",
        _ => return Err(AppError::Validation("链名称无效".into())),
    };
    app.clipboard()
        .write_text(address)
        .map_err(|_| AppError::Validation("复制失败，请重试".into()))
}
#[tauri::command]
fn test_creator_notification(app: tauri::AppHandle) -> Result<(), AppError> {
    app.notification()
        .builder()
        .title("XUnFollow · 提醒测试")
        .body("这是本机测试通知，不会打开或操作 X。")
        .show()
        .map_err(|_| AppError::Validation("通知发送失败，请检查系统通知权限".into()))
}

#[tauri::command]
fn save_workbench_preferences(
    state: State<'_, AppState>,
    preferences: Preferences,
) -> Result<(), AppError> {
    state.db.save_preferences(&preferences)
}
#[tauri::command]
fn save_reply_goal(state: State<'_, AppState>, target: i64) -> Result<(), AppError> {
    state.db.save_reply_goal(target)
}
#[tauri::command]
fn clear_mutual_cache(state: State<'_, AppState>) -> Result<(), AppError> {
    if state.reply_running.load(Ordering::Acquire) {
        return Err(AppError::Validation("请先暂停任务再刷新名单".into()));
    }
    state.db.clear_mutual_cache()
}
#[tauri::command]
fn increase_reply_x_cap(state: State<'_, AppState>, new_cap_usd: String) -> Result<(), AppError> {
    if state.reply_running.load(Ordering::Acquire) {
        return Err(AppError::Validation("请先暂停任务".into()));
    }
    state.db.increase_reply_x_cap(&new_cap_usd)?;
    let id = state.db.resume_reply_run()?;
    launch_reply_worker(&state, id)
}
#[tauri::command]
fn copy_reply_draft(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    post_id: String,
) -> Result<(), AppError> {
    let (_, draft, status) = state.db.reply_action_data(&post_id)?;
    if draft.trim().is_empty() {
        return Err(AppError::Validation("这条帖子还没有回复草稿".into()));
    }
    app.clipboard()
        .write_text(draft)
        .map_err(|_| AppError::Validation("复制失败，请重试".into()))?;
    if matches!(status.as_str(), "pending" | "copied") {
        state.db.update_reply_post(&post_id, None, Some("copied"))?;
    }
    Ok(())
}
#[tauri::command]
fn open_reply_posts(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    post_ids: Vec<String>,
) -> Result<usize, AppError> {
    if post_ids.is_empty()
        || post_ids.len() > 5
        || post_ids
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len()
            != post_ids.len()
    {
        return Err(AppError::Validation("一次选择1–5条不同的帖子".into()));
    }
    let mut items = Vec::new();
    for id in &post_ids {
        let (url, draft, status) = state.db.reply_action_data(id)?;
        items.push((id, validated_external_profile_url(&url)?, draft, status));
    }
    if items.len() == 1 && !items[0].2.trim().is_empty() {
        app.clipboard()
            .write_text(items[0].2.clone())
            .map_err(|_| AppError::Validation("自动复制失败，未打开页面；请重试".into()))?;
    }
    let mut opened = 0;
    for (id, url, draft, status) in items {
        app.opener()
            .open_url(url.as_str(), None::<&str>)
            .map_err(|_| {
                AppError::Network(format!(
                    "已打开{opened}个页面，后续打开失败；请重试剩余帖子"
                ))
            })?;
        opened += 1;
        state.db.mark_reply_opened(id)?;
        if post_ids.len() == 1
            && !draft.trim().is_empty()
            && matches!(status.as_str(), "pending" | "copied")
        {
            state.db.update_reply_post(id, None, Some("copied"))?;
        }
    }
    Ok(opened)
}

#[derive(Clone)]
struct AppState {
    db: Arc<AppDb>,
    sync_running: Arc<AtomicBool>,
    reply_running: Arc<AtomicBool>,
    reply_continue: Arc<AtomicBool>,
    creator_running: Arc<AtomicBool>,
    creator_continue: Arc<AtomicBool>,
}

#[tauri::command]
fn workbench_snapshot(state: State<'_, AppState>) -> Result<WorkbenchSnapshot, AppError> {
    state
        .db
        .workbench_snapshot(state.reply_running.load(Ordering::Acquire))
}

#[tauri::command]
fn save_reply_persona(state: State<'_, AppState>, persona: Persona) -> Result<(), AppError> {
    state.db.save_persona(&persona)
}

#[tauri::command]
fn save_deepseek_key(state: State<'_, AppState>, api_key: String) -> Result<(), AppError> {
    state.db.save_deepseek_key(&api_key)
}

fn launch_reply_worker(state: &AppState, id: i64) -> Result<(), AppError> {
    if state.reply_running.swap(true, Ordering::AcqRel) {
        return Err(AppError::Validation("回复任务已经在运行".into()));
    }
    let db = state.db.clone();
    let running = state.reply_running.clone();
    state.reply_continue.store(true, Ordering::Release);
    let keep_running = state.reply_continue.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(error) = workbench::run_reply_batch(&db, id, &keep_running).await {
            let _ = db.set_phase(id, "paused", Some(&error.to_string()));
        }
        running.store(false, Ordering::Release);
    });
    Ok(())
}

#[tauri::command]
fn start_reply_run(state: State<'_, AppState>, config: ReplyConfig) -> Result<(), AppError> {
    if state.reply_running.load(Ordering::Acquire) {
        return Err(AppError::Validation("回复任务已经在运行".into()));
    }
    let id = state.db.create_reply_run(&config)?;
    launch_reply_worker(&state, id)
}

#[tauri::command]
fn resume_reply_run(
    state: State<'_, AppState>,
    acknowledge_uncertain_cost: bool,
) -> Result<(), AppError> {
    if !acknowledge_uncertain_cost {
        return Err(AppError::Validation("请确认可能发生的重复计费".into()));
    }
    if state.reply_running.load(Ordering::Acquire) {
        return Err(AppError::Validation("回复任务已经在运行".into()));
    }
    let id = state.db.resume_reply_run()?;
    launch_reply_worker(&state, id)
}

#[tauri::command]
fn stop_reply_run(state: State<'_, AppState>) {
    state.reply_continue.store(false, Ordering::Release);
}

#[tauri::command]
fn update_reply_post(
    state: State<'_, AppState>,
    post_id: String,
    draft: Option<String>,
    status: Option<String>,
) -> Result<(), AppError> {
    state
        .db
        .update_reply_post(&post_id, draft.as_deref(), status.as_deref())
}

#[tauri::command]
fn regenerate_reply_post(state: State<'_, AppState>, post_id: String) -> Result<(), AppError> {
    if state.reply_running.load(Ordering::Acquire) {
        return Err(AppError::Validation(
            "请等待当前准备任务结束或先暂停".into(),
        ));
    }
    let id = state.db.queue_regeneration(&post_id)?;
    if !state.reply_running.load(Ordering::Acquire) {
        launch_reply_worker(&state, id)?;
    }
    Ok(())
}

#[tauri::command]
fn increase_reply_ai_cap(state: State<'_, AppState>, new_cap_usd: String) -> Result<(), AppError> {
    if state.reply_running.load(Ordering::Acquire) {
        return Err(AppError::Validation("请先暂停当前任务".into()));
    }
    let id = state.db.increase_reply_ai_cap(&new_cap_usd)?;
    launch_reply_worker(&state, id)
}

#[tauri::command]
async fn create_article_draft(
    state: State<'_, AppState>,
    topic: String,
    ai_cap_usd: String,
) -> Result<String, AppError> {
    workbench::generate_article(&state.db, &topic, &ai_cap_usd).await
}

#[tauri::command]
fn save_article_draft(
    state: State<'_, AppState>,
    topic: String,
    body: String,
) -> Result<(), AppError> {
    state.db.save_article(&topic, &body)
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
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            let path = app.path().app_data_dir()?.join("xunfollow.sqlite3");
            app.manage(AppState {
                db: Arc::new(AppDb::new(path)?),
                sync_running: Arc::new(AtomicBool::new(false)),
                reply_running: Arc::new(AtomicBool::new(false)),
                reply_continue: Arc::new(AtomicBool::new(false)),
                creator_running: Arc::new(AtomicBool::new(false)),
                creator_continue: Arc::new(AtomicBool::new(false)),
            });
            use tauri::{menu::{Menu,MenuItem},tray::TrayIconBuilder};
            let open=MenuItem::with_id(app,"open","打开 XUnFollow",true,None::<&str>)?;
            let quit=MenuItem::with_id(app,"quit","退出（停止提醒）",true,None::<&str>)?;
            let menu=Menu::with_items(app,&[&open,&quit])?;
            let mut tray=TrayIconBuilder::with_id("creator-tray").tooltip("XUnFollow · 本地发布提醒").menu(&menu).on_menu_event(|app,event|match event.id.as_ref(){
                "open"=>{if let Some(w)=app.get_webview_window("main"){let _=w.unminimize();let _=w.show();let _=w.set_focus();}},
                "quit"=>app.exit(0),_=>{}
            });
            if let Some(icon)=app.default_window_icon(){tray=tray.icon(icon.clone());}
            tray.build(app)?;
            let handle=app.handle().clone(); let db=app.state::<AppState>().db.clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                    if let Ok(p)=db.preferences(){if p.reminders_enabled {if let Ok(ids)=db.creator_due(){if !ids.is_empty(){
                        let mut n=handle.notification().builder().title("XUnFollow · 到点发布").body(format!("有 {} 篇审核通过的内容到了计划时间。请打开创作工作台，人工发布后标记完成。",ids.len()));
                        if p.reminder_sound{n=n.sound("default");}
                        if n.show().is_ok(){let _=db.mark_creator_notified(&ids);}
                    }}}}
                }
            });
            Ok(())
        })
        .on_window_event(|window,event|{if let tauri::WindowEvent::CloseRequested{api,..}=event{if window.app_handle().tray_by_id("creator-tray").is_some(){api.prevent_close();let _=window.hide();}}})
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
            workbench_snapshot,
            save_reply_persona,
            save_deepseek_key,
            start_reply_run,
            resume_reply_run,
            stop_reply_run,
            update_reply_post,
            regenerate_reply_post,
            increase_reply_ai_cap,
            create_article_draft,
            save_article_draft,
            save_workbench_preferences,
            save_workbench_settings,
            save_reply_goal,
            clear_mutual_cache,
            increase_reply_x_cap,
            copy_reply_draft,
            open_reply_posts,
            creator_snapshot,save_creator_goal,start_creator_run,restart_creator_run,creator_reference_snapshot,read_creator_reference,save_creator_reference_guide,save_creator_reference_selection,start_screenshot_run,resume_creator_run,stop_creator_run,end_creator_run,raise_creator_cap,edit_creator_draft,set_creator_status,approve_creator_run,plan_creator,snooze_creator,creator_calendar,copy_creator_draft,rewrite_creator_draft,undo_creator_rewrite,copy_support_address,test_creator_notification,
        ])
        .build(tauri::generate_context!())
        .expect("error while building XUnFollow")
        .run(|app,event|{
            #[cfg(target_os="macos")]
            if let tauri::RunEvent::Reopen{..}=event {if let Some(w)=app.get_webview_window("main"){let _=w.unminimize();let _=w.show();let _=w.set_focus();}}
            #[cfg(not(target_os="macos"))]
            let _=(app,event);
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn creator_start_gate_blocks_double_clicks_before_changing_checkpoints() {
        let dir = std::env::temp_dir().join(format!(
            "xunfollow-start-gate-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ));
        let state = AppState {
            db: Arc::new(AppDb::new(dir.join("test.sqlite3")).unwrap()),
            sync_running: Arc::new(AtomicBool::new(false)),
            reply_running: Arc::new(AtomicBool::new(false)),
            reply_continue: Arc::new(AtomicBool::new(false)),
            creator_running: Arc::new(AtomicBool::new(true)),
            creator_continue: Arc::new(AtomicBool::new(false)),
        };
        let prepared = AtomicBool::new(false);
        assert!(launch_creator_worker(&state, |_| {
            prepared.store(true, Ordering::Release);
            Ok(())
        })
        .is_err());
        assert!(!prepared.load(Ordering::Acquire));
        assert!(state.creator_running.load(Ordering::Acquire));
        state.creator_running.store(false, Ordering::Release);
        assert!(launch_creator_worker(&state, |_| Err(AppError::Validation(
            "test-only validation failure".into()
        )))
        .is_err());
        assert!(!state.creator_running.load(Ordering::Acquire));
        std::fs::remove_dir_all(dir).unwrap();
    }
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
