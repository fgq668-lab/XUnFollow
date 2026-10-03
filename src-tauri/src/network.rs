use crate::{
    db::AppDb,
    error::AppError,
    provider::{ids_page_cost, profile_page_cost, TwitterApiIo},
};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    sync::atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Default, Serialize, Deserialize)]
pub(crate) struct MutualNetwork {
    pub followers: BTreeSet<String>,
    pub following_ids: BTreeSet<String>,
    pub handles: Vec<String>,
    pub followers_done: bool,
    pub complete: bool,
    pub cursor: String,
    pub seen: BTreeSet<String>,
    pub pages: usize,
    pub captured_at: Option<String>,
}
impl AppDb {
    pub(crate) fn load_network(&self, key: &str) -> Result<Option<MutualNetwork>, AppError> {
        self.with_connection(|conn| {
            let raw: Option<String> = conn
                .query_row("SELECT value FROM settings WHERE key=?1", [key], |r| {
                    r.get(0)
                })
                .optional()?;
            raw.map(|s| {
                serde_json::from_str(&s).map_err(|_| AppError::Validation("互关缓存损坏".into()))
            })
            .transpose()
        })
    }
    fn commit_network_page(
        &self,
        id: i64,
        network: &MutualNetwork,
        cost: i64,
        owner: &str,
    ) -> Result<(), AppError> {
        let raw = serde_json::to_string(network)
            .map_err(|_| AppError::Validation("互关缓存保存失败".into()))?;
        self.with_connection(|conn| {
            let tx=conn.transaction()?;
            tx.execute("INSERT INTO settings(key,value) VALUES (?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",params![format!("reply_network:{id}"),raw])?;
            if network.complete {
                tx.execute("INSERT INTO settings(key,value) VALUES (?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",params![format!("mutual_cache:{}",owner.to_ascii_lowercase()),raw])?;
            }
            tx.execute("UPDATE reply_runs SET x_spent_micros=x_spent_micros+?1,in_flight=NULL,updated_at=?2 WHERE id=?3",params![cost,chrono::Local::now().to_rfc3339(),id])?;
            tx.commit()?; Ok(())
        })
    }
    pub fn clear_mutual_cache(&self) -> Result<(), AppError> {
        let owner = self.preferences()?.own_username;
        self.with_connection(|conn| {
            conn.execute(
                "DELETE FROM settings WHERE key=?1",
                [format!("mutual_cache:{}", owner.to_ascii_lowercase())],
            )?;
            Ok(())
        })
    }
}
fn advance(network: &mut MutualNetwork, has_next: bool, cursor: &str) -> Result<(), AppError> {
    if has_next
        && (cursor.is_empty() || cursor == network.cursor || !network.seen.insert(cursor.into()))
    {
        return Err(AppError::Provider(
            "互关名单分页游标无效或重复，已停止".into(),
        ));
    }
    network.pages += 1;
    if has_next {
        network.cursor = cursor.into();
    } else if !network.followers_done {
        network.followers_done = true;
        network.cursor.clear();
        network.seen.clear();
    } else {
        network.complete = true;
        network.captured_at = Some(chrono::Local::now().to_rfc3339());
    }
    Ok(())
}
pub(crate) async fn ensure_mutual(
    db: &AppDb,
    id: i64,
    owner: &str,
    running: &AtomicBool,
) -> Result<Option<Vec<String>>, AppError> {
    let provider = TwitterApiIo::new(db.load_api_key()?)?;
    ensure_mutual_with_provider(db, id, owner, running, &provider).await
}
async fn ensure_mutual_with_provider(
    db: &AppDb,
    id: i64,
    owner: &str,
    running: &AtomicBool,
    provider: &TwitterApiIo,
) -> Result<Option<Vec<String>>, AppError> {
    if let Some(cache) = db.load_network(&format!("mutual_cache:{}", owner.to_ascii_lowercase()))? {
        if cache.complete {
            return Ok(Some(cache.handles));
        }
    }
    let mut network = db
        .load_network(&format!("reply_network:{id}"))?
        .unwrap_or_default();
    while !network.complete {
        if !running.load(Ordering::Acquire) {
            db.set_phase(id, "paused", None)?;
            return Ok(None);
        }
        if network.pages >= 10000
            || network.followers.len() > 1_000_000
            || network.following_ids.len() > 1_000_000
        {
            return Err(AppError::Validation("互关名单超过安全边界".into()));
        }
        let reservation = if network.followers_done {
            4_000
        } else {
            40_000
        };
        if let Err(error) = db.reserve(id, "search", "network", reservation) {
            db.set_phase(
                id,
                "paused",
                Some(&format!(
                    "互关名单采集暂停：{error}；提高本轮搜索上限后继续"
                )),
            )?;
            return Ok(None);
        }
        let result = async {
            let cost;
            if !network.followers_done {
                let page = provider.follower_ids(owner, &network.cursor).await?;
                if page.ids.len() > 5000 {
                    return Err(AppError::Provider("关注者响应超出单页安全边界".into()));
                }
                cost = ids_page_cost(page.ids.len())?;
                network.followers.extend(page.ids);
                advance(&mut network, page.has_next, &page.next_cursor)?;
            } else {
                let page = provider.followings(owner, &network.cursor).await?;
                if page.profiles.len() > 200 {
                    return Err(AppError::Provider("正在关注响应超出单页安全边界".into()));
                }
                cost = profile_page_cost(page.profiles.len())?;
                for profile in page.profiles {
                    if network.following_ids.insert(profile.stable_x_id.clone())
                        && network.followers.contains(&profile.stable_x_id)
                    {
                        network.handles.push(profile.username.to_ascii_lowercase());
                    }
                }
                advance(&mut network, page.has_next, &page.next_cursor)?;
            }
            db.commit_network_page(id, &network, cost, owner)
        }
        .await;
        if let Err(error) = result {
            db.uncertain(id, &error.to_string(), true)?;
            return Ok(None);
        }
    }
    Ok(Some(network.handles))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn mock_network_verifies_ids_caches_results_and_accounts_costs() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            for (path, body) in [
                (
                    "/twitter/user/followers_ids",
                    serde_json::json!({"ids":["1","2"],"has_next_page":false}),
                ),
                (
                    "/twitter/user/followings",
                    serde_json::json!({"followings":[{"id":"1","userName":"friend","name":"Friend","followers":1,"following":1},{"id":"3","userName":"stranger","name":"Stranger","followers":1,"following":1}],"has_next_page":false}),
                ),
            ] {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(std::time::Duration::from_secs(3)))
                    .unwrap();
                let mut data = [0; 4096];
                let n = stream.read(&mut data).unwrap();
                assert!(String::from_utf8_lossy(&data[..n]).contains(path));
                let raw = body.to_string();
                write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",raw.len(),raw).unwrap();
            }
        });
        let dir = std::env::temp_dir().join(format!(
            "xunfollow-network-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ));
        let db = AppDb::new(dir.join("db.sqlite3")).unwrap();
        db.save_api_key("fixture-twitter").unwrap();
        db.save_deepseek_key("fixture-ds").unwrap();
        db.save_persona(&crate::workbench::Persona {
            identity: "builder".into(),
            language: "en".into(),
            ..Default::default()
        })
        .unwrap();
        let cfg = crate::workbench::ReplyConfig {
            keywords: vec![],
            target_count: 1,
            language: "all".into(),
            lookback_hours: 24,
            sort_mode: "latest".into(),
            x_cap_usd: "0.05".into(),
            ai_cap_usd: "0.01".into(),
            own_username: "me".into(),
            scope: "mutual".into(),
        };
        let id = db.create_reply_run(&cfg).unwrap();
        let provider = TwitterApiIo::with_test_base_url(format!("http://{address}"));
        let running = AtomicBool::new(true);
        assert_eq!(
            ensure_mutual_with_provider(&db, id, "me", &running, &provider)
                .await
                .unwrap()
                .unwrap(),
            vec!["friend"]
        );
        server.join().unwrap();
        let run = db.workbench_snapshot(false).unwrap().run.unwrap();
        assert_eq!(run.x_spent_usd, "0.001600");
        assert!(!run.needs_explicit_retry);
        assert_eq!(
            ensure_mutual_with_provider(&db, id, "me", &running, &provider)
                .await
                .unwrap()
                .unwrap(),
            vec!["friend"]
        );
        db.clear_mutual_cache().unwrap(); // unrelated owner cannot delete another account's cache
        assert!(db.load_network("mutual_cache:me").unwrap().is_some());
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn network_only_completes_after_both_lists_and_rejects_cycles() {
        let mut n = MutualNetwork::default();
        advance(&mut n, true, "a").unwrap();
        assert!(advance(&mut n, true, "a").is_err());
        advance(&mut n, false, "").unwrap();
        assert!(n.followers_done && !n.complete);
        advance(&mut n, false, "").unwrap();
        assert!(n.complete && n.captured_at.is_some());
    }
}
