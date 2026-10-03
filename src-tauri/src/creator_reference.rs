//! Explicit one-page public reads, cached locally; never operates an X page.
use crate::{db::AppDb, error::AppError, workbench};
use chrono::Utc;
use reqwest::Client;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;

const PAGE_RESERVE: i64 = 3000;
fn invalid(text: &str) -> AppError {
    AppError::Validation(text.into())
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferencePost {
    pub id: String,
    pub text: String,
    pub url: String,
    pub created_at: String,
    pub likes: i64,
    pub replies: i64,
    pub views: i64,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BloggerReference {
    pub username: String,
    pub captured_at: String,
    pub posts: Vec<ReferencePost>,
    pub median_length: usize,
    pub multiline_count: usize,
    pub filtered_count: usize,
    pub guide: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceRead {
    pub username: String,
    pub phase: String,
    pub spent_micros: i64,
    pub uncertain_micros: i64,
    pub error: Option<String>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceSnapshot {
    pub references: Vec<BloggerReference>,
    pub last_read: Option<ReferenceRead>,
    pub reading: bool,
    pub active_handles: Vec<String>,
}

pub fn handle(input: &str) -> Result<String, AppError> {
    let value = input.trim();
    let value = if value.starts_with("https://") {
        let url = url::Url::parse(value).map_err(|_| invalid("请输入 X 用户名或主页链接"))?;
        if !matches!(url.host_str(), Some("x.com" | "www.x.com"))
            || url.query().is_some()
            || url.fragment().is_some()
            || url.path().trim_matches('/').contains('/')
        {
            return Err(invalid("只支持 X 主页链接或用户名"));
        }
        url.path().trim_matches('/').to_owned()
    } else {
        value.trim_start_matches('@').to_owned()
    };
    if value.is_empty()
        || value.len() > 15
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        return Err(invalid("用户名需要1–15位字母、数字或下划线"));
    }
    Ok(value.to_ascii_lowercase())
}
pub fn initialise(db: &AppDb) -> Result<(), AppError> {
    db.with_connection(|c| {
        c.execute_batch("CREATE TABLE IF NOT EXISTS creator_references(username TEXT PRIMARY KEY, reference_json TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS creator_reference_reads(id INTEGER PRIMARY KEY AUTOINCREMENT,username TEXT NOT NULL,phase TEXT NOT NULL,spent_micros INTEGER NOT NULL DEFAULT 0,uncertain_micros INTEGER NOT NULL DEFAULT 0,error_text TEXT);
        UPDATE creator_reference_reads SET phase='uncertain',uncertain_micros=3000,error_text='上次读取被中断，可能已计费；旧缓存保留，不会自动重试' WHERE phase='reading';")?;
        Ok(())
    })
}
impl AppDb {
    pub fn reference_snapshot(&self) -> Result<ReferenceSnapshot, AppError> {
        self.with_connection(|c| {
            let mut q=c.prepare("SELECT reference_json FROM creator_references ORDER BY username")?;
            let raw=q.query_map([],|r|r.get::<_,String>(0))?.collect::<Result<Vec<_>,_>>()?;
            let references=raw.into_iter().map(|r|serde_json::from_str(&r).map_err(|_|invalid("参考博主缓存损坏，请保留数据库"))).collect::<Result<Vec<_>,_>>()?;
            let last_read=c.query_row("SELECT username,phase,spent_micros,uncertain_micros,error_text FROM creator_reference_reads ORDER BY id DESC LIMIT 1",[],|r|Ok(ReferenceRead{username:r.get(0)?,phase:r.get(1)?,spent_micros:r.get(2)?,uncertain_micros:r.get(3)?,error:r.get(4)?})).optional()?;
            let reading=last_read.as_ref().is_some_and(|r|r.phase=="reading");
            let raw:Option<String>=c.query_row("SELECT value FROM settings WHERE key='creator_reference_selection'",[],|r|r.get(0)).optional()?;
            let active_handles=raw.map(|v|serde_json::from_str(&v).map_err(|_|invalid("参考选择缓存损坏"))).transpose()?.unwrap_or_default();
            Ok(ReferenceSnapshot{references,last_read,reading,active_handles})
        })
    }
    pub fn save_reference_selection(&self, handles: &[String]) -> Result<(), AppError> {
        let selected = self
            .chosen_references(handles)?
            .into_iter()
            .map(|r| r.username)
            .collect::<Vec<_>>();
        self.with_connection(|c| {
            c.execute("INSERT INTO settings(key,value) VALUES ('creator_reference_selection',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[serde_json::to_string(&selected).map_err(|_|invalid("参考选择无法保存"))?])?;Ok(())
        })
    }
    pub fn save_reference_guide(&self, username: &str, guide: &str) -> Result<(), AppError> {
        let username = handle(username)?;
        if guide.chars().count() > 1500 {
            return Err(invalid("参考规则最多1500字"));
        }
        self.with_connection(|c| {
            let raw: String = c.query_row(
                "SELECT reference_json FROM creator_references WHERE username=?1",
                [&username],
                |r| r.get(0),
            )?;
            let mut reference: BloggerReference =
                serde_json::from_str(&raw).map_err(|_| invalid("参考缓存损坏"))?;
            reference.guide = guide.trim().to_owned();
            c.execute(
                "UPDATE creator_references SET reference_json=?1 WHERE username=?2",
                params![
                    serde_json::to_string(&reference).map_err(|_| invalid("参考缓存无法保存"))?,
                    username
                ],
            )?;
            Ok(())
        })
    }
    pub fn chosen_references(&self, handles: &[String]) -> Result<Vec<BloggerReference>, AppError> {
        if handles.len() > 2 {
            return Err(invalid("每次最多参考2个博主，避免风格混杂"));
        }
        let cached = self.reference_snapshot()?.references;
        let mut seen = BTreeSet::new();
        handles
            .iter()
            .map(|name| {
                let name = handle(name)?;
                if !seen.insert(name.clone()) {
                    return Err(invalid("参考博主不能重复"));
                }
                cached
                    .iter()
                    .find(|r| r.username == name)
                    .cloned()
                    .ok_or_else(|| invalid("请先读取这个博主的公开帖子"))
            })
            .collect()
    }
    fn begin_reference(&self, username: &str, cap: &str, ack: bool) -> Result<i64, AppError> {
        if workbench::parse_cap(cap)? < PAGE_RESERVE {
            return Err(invalid("单页保守预留 $0.003，请提高本次读取上限"));
        }
        self.with_connection(|c| {
            let tx=c.transaction()?;
            let pending:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM creator_reference_reads WHERE phase='reading')",[],|r|r.get(0))?;
            if pending {return Err(invalid("参考读取正在进行，请等待，不会重复请求"));}
            let uncertain:i64=tx.query_row("SELECT COALESCE((SELECT uncertain_micros FROM creator_reference_reads ORDER BY id DESC LIMIT 1),0)",[],|r|r.get(0))?;
            if uncertain>0 && !ack {return Err(invalid("上次读取可能已计费，请明确确认后手动重试"));}
            tx.execute("INSERT INTO creator_reference_reads(username,phase) VALUES (?1,'reading')",[username])?;
            let id=tx.last_insert_rowid();tx.commit()?;Ok(id)
        })
    }
    #[cfg(test)]
    pub fn cache_reference_response(
        &self,
        username: &str,
        body: &str,
    ) -> Result<BloggerReference, AppError> {
        let username = handle(username)?;
        let (reference, _) = parse(&username, body)?;
        self.store_reference(&reference)?;
        Ok(reference)
    }
    fn store_reference(&self, reference: &BloggerReference) -> Result<(), AppError> {
        if reference.posts.is_empty() {
            return Err(invalid(
                "本页没有合格的中文原创帖，旧参考仍保留；没有自动翻页",
            ));
        }
        self.with_connection(|c| {
            c.execute("INSERT INTO creator_references(username,reference_json) VALUES (?1,?2) ON CONFLICT(username) DO UPDATE SET reference_json=excluded.reference_json",params![reference.username,serde_json::to_string(reference).map_err(|_|invalid("参考缓存无法保存"))?])?;
            Ok(())
        })
    }
}

fn parse(username: &str, body: &str) -> Result<(BloggerReference, usize), AppError> {
    let v: Value = serde_json::from_str(body).map_err(|_| {
        AppError::Provider("参考博主响应不是有效 JSON，可能计费，没有自动重试".into())
    })?;
    if v["status"].as_str().is_some_and(|s| s != "success") {
        return Err(AppError::Provider(
            "参考读取未成功，请检查 Provider 额度与用户名".into(),
        ));
    }
    let tweets = v
        .get("tweets")
        .or_else(|| v.pointer("/data/tweets"))
        .and_then(Value::as_array)
        .ok_or_else(|| AppError::Provider("参考响应缺少帖子列表，可能计费，已停止".into()))?;
    if tweets.len() > 20 {
        return Err(AppError::Provider(
            "Provider 返回超出文档单页20条上限，计费不确定，已停止".into(),
        ));
    }
    let mut seen = BTreeSet::new();
    let posts = tweets
        .iter()
        .filter_map(|t| {
            let author = t.pointer("/author/userName").and_then(Value::as_str)?;
            let id = t["id"].as_str()?;
            let text = t["text"].as_str()?.trim();
            if !author.eq_ignore_ascii_case(username)
                || id.is_empty()
                || !id.bytes().all(|b| b.is_ascii_digit())
                || !seen.insert(id.to_owned())
                || text.chars().count() > 4000
                || !text.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c))
                || t["isReply"].as_bool().unwrap_or(false)
                || t.get("retweeted_tweet").is_some_and(|v| !v.is_null())
                || text.starts_with("RT @")
                || unsuitable(text)
            {
                return None;
            }
            Some(ReferencePost {
                id: id.into(),
                text: text.into(),
                url: format!("https://x.com/{username}/status/{id}"),
                created_at: t["createdAt"].as_str().unwrap_or("").into(),
                likes: t["likeCount"].as_i64().unwrap_or(0).max(0),
                replies: t["replyCount"].as_i64().unwrap_or(0).max(0),
                views: t["viewCount"].as_i64().unwrap_or(0).max(0),
            })
        })
        .collect::<Vec<_>>();
    let mut lengths = posts
        .iter()
        .map(|p| p.text.chars().count())
        .collect::<Vec<_>>();
    lengths.sort();
    let median_length = lengths.get(lengths.len() / 2).copied().unwrap_or(0);
    let multiline_count = posts.iter().filter(|p| p.text.contains('\n')).count();
    let guide=format!("只借鉴这批样本的日常用词、开场切入和长短节奏；样本中位长度约{median_length}字符（含链接），{multiline_count}/{}条有换行，不要每篇套一样的模板。保持我自己的身份和语气，不移植博主经历、关系、观点或事实，不抄句子、不假装认识他。配图文案可能缺少语境，别编图片内容；不学粗口、人身攻击或成人引流，也不把互动量当内容真实性。",posts.len());
    let filtered_count = tweets.len() - posts.len();
    Ok((
        BloggerReference {
            username: username.into(),
            captured_at: Utc::now().to_rfc3339(),
            posts,
            median_length,
            multiline_count,
            filtered_count,
            guide,
        },
        tweets.len(),
    ))
}
fn unsuitable(text: &str) -> bool {
    let lower = text.to_lowercase();
    [
        "妈逼",
        "你妈",
        "日你",
        "黑鬼",
        "av ",
        " av",
        "番号",
        "h 文",
        "h文",
        "裸奔",
        "十八个币",
        "我 tm",
        "主人",
        "各种幻想",
        "性爱",
        "性服务",
    ]
    .iter()
    .any(|word| lower.contains(word))
}

pub async fn read(
    db: &AppDb,
    input: &str,
    cap: &str,
    ack: bool,
) -> Result<BloggerReference, AppError> {
    let username = handle(input)?;
    let key = db.load_api_key()?;
    let client = Client::builder()
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(45))
        .build()
        .map_err(|_| invalid("无法建立读取连接"))?;
    let id = db.begin_reference(&username, cap, ack)?;
    let result = async {
        let response = client
            .get("https://api.twitterapi.io/twitter/user/last_tweets")
            .header("X-API-Key", &key)
            .query(&[
                ("userName", username.as_str()),
                ("includeReplies", "false"),
                ("cursor", ""),
            ])
            .send()
            .await
            .map_err(|_| {
                AppError::Network("参考请求失败，可能计费，旧缓存保留；不会自动重试".into())
            })?;
        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|_| AppError::Network("参考响应中断，可能计费；不会自动重试".into()))?;
        if !status.is_success() {
            return Err(workbench::safe_provider_error(status, &body, &key));
        }
        parse(&username, &body)
    }
    .await;
    match result {
        Ok((reference, count)) => {
            let cost = count.max(1) as i64 * 150;
            db.with_connection(|c| {
                c.execute(
                    "UPDATE creator_reference_reads SET phase='done',spent_micros=?1 WHERE id=?2",
                    params![cost, id],
                )?;
                Ok(())
            })?;
            db.store_reference(&reference)?;
            Ok(reference)
        }
        Err(error) => {
            db.with_connection(|c|{c.execute("UPDATE creator_reference_reads SET phase='uncertain',uncertain_micros=?1,error_text=?2 WHERE id=?3",params![PAGE_RESERVE,error.to_string(),id])?;Ok(())})?;
            Err(error)
        }
    }
}

/// Examples are user-role data, never instructions or factual material for the new post.
pub fn examples(references: &[BloggerReference]) -> Value {
    serde_json::json!(references.iter().map(|r|{
        let mut chosen=r.posts.iter().collect::<Vec<_>>();chosen.sort_by_key(|p|std::cmp::Reverse(p.likes));
        let top=chosen.into_iter().take(3).chain(r.posts.iter().take(3));
        let mut seen=BTreeSet::new();
        let posts=top.filter(|p|seen.insert(&p.id)).map(|p|serde_json::json!({"textStyleOnly":p.text.chars().take(800).collect::<String>(),"url":p.url})).collect::<Vec<_>>();
        serde_json::json!({"username":r.username,"guide":r.guide,"examplesStyleOnly":posts})
    }).collect::<Vec<_>>())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (AppDb, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "xunfollow-reference-{}-{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap()
        ));
        let db = AppDb::new(dir.join("test.sqlite3")).unwrap();
        (db, dir)
    }
    #[test]
    fn reference_budget_gate_and_restart_keep_uncertainty_without_auto_retry() {
        let (db, dir) = fixture();
        assert!(db.begin_reference("example", "0.002", false).is_err());
        assert!(db.reference_snapshot().unwrap().last_read.is_none());
        db.begin_reference("example", "0.003", false).unwrap();
        assert!(db.reference_snapshot().unwrap().reading);
        assert!(db.begin_reference("example", "0.003", false).is_err());
        initialise(&db).unwrap();
        let s = db.reference_snapshot().unwrap();
        assert!(!s.reading);
        assert_eq!(s.last_read.unwrap().uncertain_micros, 3000);
        assert!(db.begin_reference("example", "0.003", false).is_err());
        db.begin_reference("example", "0.003", true).unwrap();
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn cache_guide_bounded_failures_preserve_old_data_and_no_credentials_echo() {
        let (db, dir) = fixture();
        db.save_api_key("private-reference-fixture-secret").unwrap();
        let raw=serde_json::json!({"tweets":[{"id":"123","text":"先说一个小细节，再空一行。","author":{"userName":"example"}}]}).to_string();
        db.cache_reference_response("example", &raw).unwrap();
        db.save_reference_guide("example", "只参考句子节奏，不照搬内容")
            .unwrap();
        assert!(db
            .save_reference_guide("example", &"长".repeat(1501))
            .is_err());
        assert!(db
            .cache_reference_response("example", r#"{"tweets":[]}"#)
            .is_err());
        let s = db.reference_snapshot().unwrap();
        assert_eq!(s.references[0].posts.len(), 1);
        assert_eq!(s.references[0].guide, "只参考句子节奏，不照搬内容");
        assert!(!serde_json::to_string(&s)
            .unwrap()
            .contains("private-reference-fixture-secret"));
        assert!(db.chosen_references(&vec!["example".into(); 3]).is_err());
        assert!(db
            .chosen_references(&["example".into(), "example".into()])
            .is_err());
        assert!(db.chosen_references(&["missing".into()]).is_err());
        db.save_reference_selection(&["@Example".into()]).unwrap();
        assert_eq!(
            db.reference_snapshot().unwrap().active_handles,
            vec!["example"]
        );
        assert!(db.save_reference_selection(&["missing".into()]).is_err());
        assert_eq!(
            db.reference_snapshot().unwrap().active_handles,
            vec!["example"]
        );
        db.save_reference_selection(&[]).unwrap();
        assert!(db.reference_snapshot().unwrap().active_handles.is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn unsafe_examples_are_not_sent_and_payload_is_bounded() {
        let tweets=(0..20).map(|i|serde_json::json!({"id":(100+i).to_string(),"text":if i==0{"不适合的番号引流".into()}else{"日常细节".repeat(200)},"author":{"userName":"example"}})).collect::<Vec<_>>();
        let (r, count) =
            parse("example", &serde_json::json!({"tweets":tweets}).to_string()).unwrap();
        assert_eq!(count, 20);
        assert_eq!(r.filtered_count, 1);
        let p = examples(&[r]);
        assert!(p[0]["examplesStyleOnly"].as_array().unwrap().len() <= 6);
        assert!(!p.to_string().contains("番号"));
        for e in p[0]["examplesStyleOnly"].as_array().unwrap() {
            assert!(e["textStyleOnly"].as_str().unwrap().chars().count() <= 800);
        }
    }
    #[test]
    fn handles_only_allow_profiles_and_not_operator_injection() {
        assert_eq!(handle("https://x.com/xupaopaogm/").unwrap(), "xupaopaogm");
        assert_eq!(handle("@Example").unwrap(), "example");
        for bad in [
            "https://x.com.evil.test/foo",
            "https://x.com/foo/status/1",
            "https://x.com/foo?x=y",
            "a OR b",
            "https://evil.test/x",
        ] {
            assert!(handle(bad).is_err());
        }
    }
    #[test]
    fn parses_both_shapes_charges_all_returns_but_only_keeps_original_chinese() {
        let original = serde_json::json!({"id":"123","text":"写一个小细节。\n\n讲完就停。","author":{"userName":"example"},"likeCount":4});
        let english =
            serde_json::json!({"id":"124","text":"English","author":{"userName":"example"}});
        let reply = serde_json::json!({"id":"125","text":"这是回复","isReply":true,"author":{"userName":"example"}});
        let outsider =
            serde_json::json!({"id":"126","text":"外部作者内容","author":{"userName":"other"}});
        let tweets = serde_json::json!([original, english, reply, outsider]);
        for body in [
            serde_json::json!({"tweets":tweets}),
            serde_json::json!({"data":{"tweets":tweets}}),
        ] {
            let (r, count) = parse("example", &body.to_string()).unwrap();
            assert_eq!(count, 4);
            assert_eq!(r.posts.len(), 1);
            assert_eq!(r.multiline_count, 1);
            assert_eq!(r.posts[0].url, "https://x.com/example/status/123");
            assert!(examples(&[r]).to_string().contains("examplesStyleOnly"));
        }
        assert!(parse("example", r#"{"status":"error","tweets":[]}"#).is_err());
    }
}
