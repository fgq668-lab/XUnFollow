use std::{collections::BTreeMap, path::PathBuf};

use chrono::{Local, SecondsFormat};
use rusqlite::{params, Connection, OptionalExtension, Transaction};

use crate::{
    error::AppError,
    models::{
        Account, Bootstrap, Candidate, Decision, DecisionStatus, HistoryEntry, ImportedDecision,
        PendingScan, ScanSummary,
    },
};

const API_KEY_SETTING: &str = "provider_api_key";

#[derive(Clone)]
pub struct AppDb {
    path: PathBuf,
}

impl AppDb {
    pub fn new(path: impl Into<PathBuf>) -> Result<Self, AppError> {
        let db = Self { path: path.into() };
        db.with_connection(|connection| {
            connection.execute_batch(
                "PRAGMA journal_mode=WAL;
                 PRAGMA foreign_keys=ON;
                 CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
                 CREATE TABLE IF NOT EXISTS candidates (
                    stable_x_id TEXT PRIMARY KEY,
                    username TEXT NOT NULL,
                    name TEXT NOT NULL,
                    profile_image_url TEXT,
                    followers_count INTEGER NOT NULL,
                    following_count INTEGER NOT NULL,
                    x_url TEXT NOT NULL,
                    source_order INTEGER NOT NULL
                 );
                 CREATE TABLE IF NOT EXISTS decisions (
                    stable_x_id TEXT PRIMARY KEY,
                    status TEXT NOT NULL CHECK(status IN ('unfollowed','keep','later','changed')),
                    action_day TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                 );
                 CREATE TABLE IF NOT EXISTS decision_events (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    stable_x_id TEXT NOT NULL,
                    previous_status TEXT,
                    previous_action_day TEXT,
                    new_status TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    undone_at TEXT
                 );
                 CREATE INDEX IF NOT EXISTS decision_events_active_idx ON decision_events(undone_at, id DESC);
                 CREATE TABLE IF NOT EXISTS scan_checkpoints (
                    id INTEGER PRIMARY KEY CHECK(id = 1),
                    state_json TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                 );
                 CREATE TABLE IF NOT EXISTS reply_persona (
                    id INTEGER PRIMARY KEY CHECK(id = 1),
                    identity_text TEXT NOT NULL DEFAULT '',
                    topics TEXT NOT NULL DEFAULT '',
                    voice TEXT NOT NULL DEFAULT '',
                    language TEXT NOT NULL DEFAULT 'zh',
                    avoid_text TEXT NOT NULL DEFAULT ''
                 );
                 CREATE TABLE IF NOT EXISTS reply_runs (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    day TEXT NOT NULL,
                    phase TEXT NOT NULL,
                    query_text TEXT NOT NULL,
                    query_type TEXT NOT NULL,
                    target_count INTEGER NOT NULL,
                    max_pages INTEGER NOT NULL,
                    pages_done INTEGER NOT NULL DEFAULT 0,
                    cursor TEXT NOT NULL DEFAULT '',
                    x_cap_micros INTEGER NOT NULL,
                    ai_cap_micros INTEGER NOT NULL,
                    x_spent_micros INTEGER NOT NULL DEFAULT 0,
                    ai_spent_micros INTEGER NOT NULL DEFAULT 0,
                    ai_reserved_micros INTEGER NOT NULL DEFAULT 0,
                    x_uncertain_micros INTEGER NOT NULL DEFAULT 0,
                    ai_uncertain_micros INTEGER NOT NULL DEFAULT 0,
                    in_flight TEXT,
                    error_text TEXT,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                 );
                 CREATE TABLE IF NOT EXISTS reply_posts (
                    post_id TEXT PRIMARY KEY,
                    run_id INTEGER NOT NULL,
                    day TEXT NOT NULL,
                    username TEXT NOT NULL,
                    post_text TEXT NOT NULL,
                    post_url TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    score REAL NOT NULL,
                    reason TEXT NOT NULL,
                    selected INTEGER NOT NULL DEFAULT 0,
                    draft TEXT NOT NULL DEFAULT '',
                    retry_requested INTEGER NOT NULL DEFAULT 0,
                    status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending','copied','replied','skipped')),
                    generation_error TEXT,
                    updated_at TEXT NOT NULL
                 );
                 CREATE INDEX IF NOT EXISTS reply_posts_run_idx ON reply_posts(run_id, score DESC);
                 CREATE TABLE IF NOT EXISTS reply_generation_requests (
                    post_id TEXT PRIMARY KEY,
                    run_id INTEGER NOT NULL,
                    reservation_micros INTEGER NOT NULL,
                    created_at TEXT NOT NULL
                 );
                 CREATE TABLE IF NOT EXISTS article_drafts (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    topic TEXT NOT NULL,
                    body TEXT NOT NULL,
                    cost_micros INTEGER NOT NULL DEFAULT 0,
                    cap_micros INTEGER NOT NULL DEFAULT 0,
                    created_at TEXT NOT NULL
                 );
                 INSERT OR IGNORE INTO settings(key, value) VALUES ('daily_goal', '10');"
            )?;
            // CREATE TABLE IF NOT EXISTS leaves the schema of existing installs
            // unchanged. Upgrade older reply workbench databases in place.
            let has_retry_requested: bool = connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM pragma_table_info('reply_posts') WHERE name='retry_requested')",
                [],
                |row| row.get(0),
            )?;
            if !has_retry_requested {
                connection.execute_batch(
                    "ALTER TABLE reply_posts ADD COLUMN retry_requested INTEGER NOT NULL DEFAULT 0;",
                )?;
            }
            for (table, column, definition) in [
                ("reply_posts", "replied_at", "TEXT"),
                ("reply_posts", "opened_at", "TEXT"),
                ("reply_runs", "config_json", "TEXT"),
                ("reply_runs", "preferences_json", "TEXT"),
            ] {
                let exists: bool = connection.query_row(
                    &format!("SELECT EXISTS(SELECT 1 FROM pragma_table_info('{table}') WHERE name=?1)"),
                    [column], |row| row.get(0),
                )?;
                if !exists { connection.execute_batch(&format!("ALTER TABLE {table} ADD COLUMN {column} {definition};"))?; }
            }
            connection.execute_batch("UPDATE reply_posts SET replied_at=updated_at WHERE status='replied' AND replied_at IS NULL;")?;
            Ok(())
        })?;
        crate::creator::initialise(&db)?;
        crate::creator_reference::initialise(&db)?;
        Ok(db)
    }

    pub fn bootstrap(&self) -> Result<Bootstrap, AppError> {
        self.with_connection(|connection| {
            let api_key_configured = setting_text(connection, API_KEY_SETTING)?.is_some_and(|key| !key.is_empty());
            let base_goal = setting_i64(connection, "daily_goal")?.unwrap_or(10);
            let daily_key = format!("daily_goal:{}", today());
            let daily_goal = setting_i64(connection, &daily_key)?.unwrap_or(base_goal);
            let account = setting_text(connection, "account")?
                .map(|value| serde_json::from_str::<Account>(&value))
                .transpose()
                .map_err(|_| AppError::Database(rusqlite::Error::InvalidQuery))?;
            let summary = setting_text(connection, "scan_summary")?
                .map(|value| serde_json::from_str::<ScanSummary>(&value))
                .transpose()
                .map_err(|_| AppError::Database(rusqlite::Error::InvalidQuery))?;
            let mut candidates = Vec::new();
            let mut candidate_statement = connection.prepare(
                "SELECT stable_x_id, username, name, profile_image_url, followers_count, following_count, x_url
                 FROM candidates ORDER BY source_order ASC"
            )?;
            let mut rows = candidate_statement.query([])?;
            while let Some(row) = rows.next()? {
                candidates.push(Candidate {
                    stable_x_id: row.get(0)?, username: row.get(1)?, name: row.get(2)?,
                    profile_image_url: row.get(3)?, followers_count: row.get(4)?,
                    following_count: row.get(5)?, x_url: row.get(6)?,
                });
            }
            let mut decisions = BTreeMap::new();
            let mut decision_statement = connection.prepare(
                "SELECT d.stable_x_id, d.status, d.action_day, d.updated_at
                 FROM decisions d INNER JOIN candidates c ON c.stable_x_id = d.stable_x_id"
            )?;
            let mut rows = decision_statement.query([])?;
            while let Some(row) = rows.next()? {
                let status: String = row.get(1)?;
                let decision = DecisionStatus::parse(&status).ok_or(rusqlite::Error::InvalidQuery)?;
                decisions.insert(row.get(0)?, Decision { status: decision, action_day: row.get(2)?, updated_at: row.get(3)? });
            }
            let mut history = Vec::new();
            let mut history_statement = connection.prepare(
                "SELECT c.stable_x_id, c.username, c.name, d.status, d.action_day, d.updated_at
                 FROM decisions d INNER JOIN candidates c ON c.stable_x_id = d.stable_x_id
                 ORDER BY d.updated_at DESC, d.stable_x_id ASC LIMIT 100",
            )?;
            let mut rows = history_statement.query([])?;
            while let Some(row) = rows.next()? {
                let status: String = row.get(3)?;
                history.push(HistoryEntry {
                    stable_x_id: row.get(0)?,
                    username: row.get(1)?,
                    name: row.get(2)?,
                    status: DecisionStatus::parse(&status).ok_or(rusqlite::Error::InvalidQuery)?,
                    action_day: row.get(4)?,
                    updated_at: row.get(5)?,
                });
            }
            let pending_scan = connection.query_row(
                "SELECT state_json FROM scan_checkpoints WHERE id=1",
                [],
                |row| row.get::<_, String>(0),
            ).optional()?
                .and_then(|raw| pending_scan_from_checkpoint(&raw));
            Ok(Bootstrap { account, candidates, decisions, history, summary, daily_goal, batch_size: base_goal, api_key_configured, pending_scan, sync_running: false })
        })
    }

    pub fn save_api_key(&self, api_key: &str) -> Result<(), AppError> {
        let key = api_key.trim();
        if key.is_empty() || key.len() > 1024 {
            return Err(AppError::Validation("API Key 无效".into()));
        }
        self.with_connection(|connection| {
            connection.execute(
                "INSERT INTO settings(key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                params![API_KEY_SETTING, key],
            )?;
            Ok(())
        })
    }

    pub fn load_api_key(&self) -> Result<String, AppError> {
        self.with_connection(|connection| {
            setting_text(connection, API_KEY_SETTING)?
                .filter(|key| !key.is_empty())
                .ok_or_else(|| AppError::Validation("还没有保存 TwitterAPI.io API Key".into()))
        })
    }

    pub fn record_decision(&self, stable_x_id: &str, status: &str) -> Result<(), AppError> {
        let status = DecisionStatus::parse(status)
            .ok_or_else(|| AppError::Validation("处理状态无效".into()))?;
        self.with_connection(|connection| {
            let transaction = connection.transaction()?;
            let exists: bool = transaction.query_row(
                "SELECT EXISTS(SELECT 1 FROM candidates WHERE stable_x_id=?1)", [stable_x_id], |row| row.get(0)
            )?;
            if !exists { return Err(AppError::Validation("账号不在当前名单中".into())); }
            let previous: Option<(String, String, String)> = transaction.query_row(
                "SELECT status, action_day, created_at FROM decisions WHERE stable_x_id=?1", [stable_x_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            ).optional()?;
            if previous.as_ref().is_some_and(|value| value.0 == status.as_str()) { return Ok(()); }
            let now = now();
            transaction.execute(
                "INSERT INTO decision_events(stable_x_id, previous_status, previous_action_day, new_status, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![stable_x_id, previous.as_ref().map(|item| &item.0), previous.as_ref().map(|item| &item.1), status.as_str(), now.clone()],
            )?;
            transaction.execute(
                "INSERT INTO decisions(stable_x_id, status, action_day, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?4)
                 ON CONFLICT(stable_x_id) DO UPDATE SET status=excluded.status, action_day=excluded.action_day, updated_at=excluded.updated_at",
                params![stable_x_id, status.as_str(), today(), previous.map(|item| item.2).unwrap_or(now.clone())],
            )?;
            transaction.commit()?;
            Ok(())
        })
    }

    pub fn undo_last(&self) -> Result<(), AppError> {
        self.with_connection(|connection| {
            let transaction = connection.transaction()?;
            let event: Option<(i64, String, Option<String>, Option<String>)> = transaction.query_row(
                "SELECT id, stable_x_id, previous_status, previous_action_day
                 FROM decision_events WHERE undone_at IS NULL ORDER BY id DESC LIMIT 1",
                [], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            ).optional()?;
            let Some((id, stable_x_id, previous_status, previous_day)) = event else { return Ok(()); };
            if let Some(status) = previous_status {
                transaction.execute("UPDATE decisions SET status=?1, action_day=?2, updated_at=?3 WHERE stable_x_id=?4", params![status, previous_day, now(), stable_x_id])?;
            } else {
                transaction.execute("DELETE FROM decisions WHERE stable_x_id=?1", [stable_x_id])?;
            }
            transaction.execute("UPDATE decision_events SET undone_at=?1 WHERE id=?2", params![now(), id])?;
            transaction.commit()?;
            Ok(())
        })
    }

    pub fn import_decisions(
        &self,
        incoming: BTreeMap<String, ImportedDecision>,
    ) -> Result<i64, AppError> {
        if incoming.len() > 1_000_000 {
            return Err(AppError::Validation("导入记录超过安全上限".into()));
        }
        self.with_connection(|connection| {
            let transaction = connection.transaction()?;
            let mut accepted = 0_i64;
            for (stable_x_id, decision) in incoming {
                if !valid_stable_x_id(&stable_x_id) || !valid_action_day(&decision.action_day) {
                    return Err(AppError::Validation("导入文件含有无效处理记录".into()));
                }
                let exists: bool = transaction.query_row(
                    "SELECT EXISTS(SELECT 1 FROM candidates WHERE stable_x_id=?1)",
                    [&stable_x_id],
                    |row| row.get(0),
                )?;
                if !exists {
                    continue;
                }
                let timestamp = now();
                transaction.execute(
                    "INSERT INTO decisions(stable_x_id, status, action_day, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?4)
                     ON CONFLICT(stable_x_id) DO UPDATE SET status=excluded.status, action_day=excluded.action_day, updated_at=excluded.updated_at",
                    params![stable_x_id, decision.status.as_str(), decision.action_day, timestamp],
                )?;
                accepted += 1;
            }
            transaction.commit()?;
            Ok(accepted)
        })
    }

    pub fn continue_batch(&self) -> Result<i64, AppError> {
        self.with_connection(|connection| {
            let transaction = connection.transaction()?;
            let base = setting_i64_tx(&transaction, "daily_goal")?.unwrap_or(10);
            let key = format!("daily_goal:{}", today());
            let current = setting_i64_tx(&transaction, &key)?.unwrap_or(base);
            let completed: i64 = transaction.query_row(
                "SELECT COUNT(*) FROM decisions d INNER JOIN candidates c ON c.stable_x_id=d.stable_x_id WHERE d.action_day=?1", [today()], |row| row.get(0)
            )?;
            if completed < current { return Err(AppError::Validation("请先完成当前这一批".into())); }
            let next = current.checked_add(base).filter(|value| *value <= 10_000)
                .ok_or_else(|| AppError::Validation("今日任务数量超过安全上限".into()))?;
            transaction.execute(
                "INSERT INTO settings(key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                params![key, next.to_string()],
            )?;
            transaction.commit()?;
            Ok(next)
        })
    }

    pub fn replace_snapshot(
        &self,
        account: &Account,
        summary: &ScanSummary,
        candidates: &[Candidate],
    ) -> Result<(), AppError> {
        self.with_connection(|connection| {
            let transaction = connection.transaction()?;
            transaction.execute("DELETE FROM candidates", [])?;
            for (index, candidate) in candidates.iter().enumerate() {
                transaction.execute(
                    "INSERT INTO candidates(stable_x_id, username, name, profile_image_url, followers_count, following_count, x_url, source_order)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![candidate.stable_x_id, candidate.username, candidate.name, candidate.profile_image_url, candidate.followers_count, candidate.following_count, candidate.x_url, index as i64],
                )?;
            }
            set_setting(&transaction, "account", &serde_json::to_string(account).map_err(|_| rusqlite::Error::InvalidQuery)?)?;
            set_setting(&transaction, "scan_summary", &serde_json::to_string(summary).map_err(|_| rusqlite::Error::InvalidQuery)?)?;
            transaction.commit()?;
            Ok(())
        })
    }

    pub fn save_checkpoint(&self, state_json: &str) -> Result<(), AppError> {
        self.with_connection(|connection| {
            connection.execute(
                "INSERT INTO scan_checkpoints(id, state_json, updated_at) VALUES (1, ?1, ?2)
                 ON CONFLICT(id) DO UPDATE SET state_json=excluded.state_json, updated_at=excluded.updated_at",
                params![state_json, now()],
            )?;
            Ok(())
        })
    }

    pub fn load_checkpoint(&self) -> Result<Option<String>, AppError> {
        self.with_connection(|connection| {
            Ok(connection
                .query_row(
                    "SELECT state_json FROM scan_checkpoints WHERE id=1",
                    [],
                    |row| row.get(0),
                )
                .optional()?)
        })
    }

    pub fn clear_checkpoint(&self) -> Result<(), AppError> {
        self.with_connection(|connection| {
            connection.execute("DELETE FROM scan_checkpoints WHERE id=1", [])?;
            Ok(())
        })
    }

    pub(crate) fn with_connection<T>(
        &self,
        operation: impl FnOnce(&mut Connection) -> Result<T, AppError>,
    ) -> Result<T, AppError> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                AppError::Database(rusqlite::Error::ToSqlConversionFailure(Box::new(error)))
            })?;
            #[cfg(unix)]
            std::fs::set_permissions(parent, std::os::unix::fs::PermissionsExt::from_mode(0o700))
                .map_err(|error| {
                AppError::Database(rusqlite::Error::ToSqlConversionFailure(Box::new(error)))
            })?;
        }
        let mut connection = Connection::open(&self.path)?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        operation(&mut connection)
    }
}

fn setting_text(connection: &Connection, key: &str) -> Result<Option<String>, AppError> {
    Ok(connection
        .query_row("SELECT value FROM settings WHERE key=?1", [key], |row| {
            row.get(0)
        })
        .optional()?)
}

fn setting_i64(connection: &Connection, key: &str) -> Result<Option<i64>, AppError> {
    let value = setting_text(connection, key)?;
    Ok(value.and_then(|item| item.parse().ok()))
}

fn setting_i64_tx(transaction: &Transaction<'_>, key: &str) -> Result<Option<i64>, AppError> {
    let value = transaction
        .query_row("SELECT value FROM settings WHERE key=?1", [key], |row| {
            row.get::<_, String>(0)
        })
        .optional()?;
    Ok(value.and_then(|item| item.parse().ok()))
}

fn set_setting(transaction: &Transaction<'_>, key: &str, value: &str) -> Result<(), AppError> {
    transaction.execute("INSERT INTO settings(key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![key, value])?;
    Ok(())
}

fn pending_scan_from_checkpoint(raw: &str) -> Option<PendingScan> {
    let value: serde_json::Value = serde_json::from_str(raw).ok()?;
    let handle = value.get("handle")?.as_str()?.to_owned();
    let micros = value.get("hard_cap_micros")?.as_i64()?;
    if handle.is_empty() || micros < 0 {
        return None;
    }
    Some(PendingScan {
        handle,
        hard_cap_usd: format!("{}.{:06}", micros / 1_000_000, micros.rem_euclid(1_000_000)),
        needs_explicit_retry: value.get("in_flight").is_some_and(|item| !item.is_null()),
        follower_pages: value
            .get("follower_pages")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0) as usize,
        following_pages: value
            .get("following_pages")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0) as usize,
        follower_ids_loaded: value
            .get("follower_ids")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len)
            .unwrap_or(0),
        following_profiles_loaded: value
            .get("following_profiles")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len)
            .unwrap_or(0),
        followers_total: value
            .get("identity")
            .and_then(|item| item.get("followers_count"))
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(0),
        following_total: value
            .get("identity")
            .and_then(|item| item.get("following_count"))
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(0),
        follower_complete: value
            .get("follower_complete")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        following_complete: value
            .get("following_complete")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
    })
}

fn valid_stable_x_id(value: &str) -> bool {
    !value.is_empty() && value.len() <= 32 && value.bytes().all(|byte| byte.is_ascii_digit())
}

fn valid_action_day(value: &str) -> bool {
    chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d").is_ok()
}

fn today() -> String {
    Local::now().date_naive().to_string()
}
fn now() -> String {
    Local::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn fixture_db(count: usize) -> (AppDb, PathBuf) {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let sequence = TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!("xunfollow-db-test-{nonce}-{sequence}"));
        let db = AppDb::new(directory.join("test.sqlite3")).unwrap();
        let candidates = (0..count)
            .map(|index| Candidate {
                stable_x_id: format!("200000000000000{index:03}"),
                username: format!("user{index}"),
                name: format!("User {index}"),
                profile_image_url: None,
                followers_count: 1,
                following_count: 1,
                x_url: format!("https://x.com/user{index}"),
            })
            .collect::<Vec<_>>();
        let account = Account {
            username: "fixture".into(),
            name: Some("Fixture".into()),
        };
        let summary = ScanSummary {
            captured_at: Some(now()),
            followers_count: 1,
            following_count: count as i64,
            non_followback_count: count as i64,
            confirmed_cost_usd: "0.000000".into(),
            maximum_possible_cost_usd: "0.000000".into(),
            complete: true,
        };
        db.replace_snapshot(&account, &summary, &candidates)
            .unwrap();
        (db, directory)
    }

    #[test]
    fn decision_is_durable_and_undoable() {
        let (db, directory) = fixture_db(1);
        let id = "200000000000000000";
        db.record_decision(id, "unfollowed").unwrap();
        assert_eq!(
            db.bootstrap().unwrap().decisions[id].status,
            DecisionStatus::Unfollowed
        );
        db.undo_last().unwrap();
        assert!(db.bootstrap().unwrap().decisions.is_empty());
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn completed_batch_can_be_extended() {
        let (db, directory) = fixture_db(10);
        for index in 0..10 {
            db.record_decision(&format!("200000000000000{index:03}"), "unfollowed")
                .unwrap();
        }
        assert_eq!(db.continue_batch().unwrap(), 20);
        assert_eq!(db.bootstrap().unwrap().daily_goal, 20);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn exposes_only_safe_resume_metadata() {
        let (db, directory) = fixture_db(0);
        db.save_checkpoint(r#"{"handle":"fixture","hard_cap_micros":500000,"in_flight":{"endpoint":"/twitter/user/followings"}}"#)
            .unwrap();
        let pending = db.bootstrap().unwrap().pending_scan.unwrap();
        assert_eq!(pending.handle, "fixture");
        assert_eq!(pending.hard_cap_usd, "0.500000");
        assert!(pending.needs_explicit_retry);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn imports_only_matching_valid_decisions() {
        let (db, directory) = fixture_db(1);
        let imported = BTreeMap::from([
            (
                "200000000000000000".into(),
                ImportedDecision {
                    status: DecisionStatus::Keep,
                    action_day: "2026-09-01".into(),
                },
            ),
            (
                "299999999999999999".into(),
                ImportedDecision {
                    status: DecisionStatus::Later,
                    action_day: "2026-09-01".into(),
                },
            ),
        ]);
        assert_eq!(db.import_decisions(imported).unwrap(), 1);
        let decision = &db.bootstrap().unwrap().decisions["200000000000000000"];
        assert_eq!(decision.status, DecisionStatus::Keep);
        assert_eq!(decision.action_day, "2026-09-01");
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn stores_api_key_in_the_local_database() {
        let (db, directory) = fixture_db(0);
        db.save_api_key("test-local-api-key").unwrap();
        assert_eq!(db.load_api_key().unwrap(), "test-local-api-key");
        assert!(db.bootstrap().unwrap().api_key_configured);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn upgrades_legacy_workbench_without_losing_drafts_or_decisions() {
        let (db, directory) = fixture_db(1);
        db.record_decision("200000000000000000", "unfollowed")
            .unwrap();
        db.with_connection(|connection| {
            connection.execute_batch(
                "INSERT INTO reply_posts(post_id,run_id,day,username,post_text,post_url,created_at,score,reason,draft,updated_at)
                 VALUES ('legacy-post',1,'2026-10-03','author','原帖','https://x.com/author/status/1','now',1.0,'test','已保存的草稿','now');
                 ALTER TABLE reply_posts DROP COLUMN retry_requested;",
            )?;
            Ok(())
        }).unwrap();
        let upgraded = AppDb::new(directory.join("test.sqlite3")).unwrap();
        // Reopening again must remain safe after the migration has been applied.
        let upgraded = AppDb::new(upgraded.path.clone()).unwrap();
        upgraded
            .with_connection(|connection| {
                let (draft, retry): (String, i64) = connection.query_row(
                    "SELECT draft,retry_requested FROM reply_posts WHERE post_id='legacy-post'",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )?;
                assert_eq!(draft, "已保存的草稿");
                assert_eq!(retry, 0);
                connection.execute(
                    "UPDATE reply_posts SET retry_requested=1 WHERE post_id='legacy-post'",
                    [],
                )?;
                Ok(())
            })
            .unwrap();
        let snapshot = upgraded.bootstrap().unwrap();
        assert_eq!(snapshot.candidates.len(), 1);
        assert_eq!(
            snapshot.decisions["200000000000000000"].status,
            DecisionStatus::Unfollowed
        );
        std::fs::remove_dir_all(directory).unwrap();
    }
}
