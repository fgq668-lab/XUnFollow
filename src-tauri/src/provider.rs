use std::collections::HashSet;

use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    db::AppDb,
    error::AppError,
    models::{Account, Candidate, CostEstimate, ResolvedAccount, ScanSummary},
};

const BASE_URL: &str = "https://api.twitterapi.io";
const IDENTITY_MICROS: i64 = 180;
const IDS_RESERVATION_MICROS: i64 = 40_000;
const PROFILE_RESERVATION_MICROS: i64 = 4_000;
const MAX_ITEMS: usize = 1_000_000;
const MAX_FOLLOWER_PAGES: usize = 1_000;
const MAX_FOLLOWING_PAGES: usize = 10_000;

#[derive(Clone)]
pub struct TwitterApiIo {
    client: Client,
    api_key: String,
    base_url: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Checkpoint {
    format: String,
    handle: String,
    hard_cap_micros: i64,
    confirmed_micros: i64,
    uncertain_micros: i64,
    identity: Option<ResolvedAccount>,
    follower_ids: Vec<String>,
    follower_cursor: String,
    follower_complete: bool,
    follower_pages: usize,
    follower_seen_cursors: Vec<String>,
    following_profiles: Vec<Candidate>,
    following_cursor: String,
    following_complete: bool,
    following_pages: usize,
    following_seen_cursors: Vec<String>,
    in_flight: Option<InFlight>,
    #[serde(default)]
    ambiguous_retry_used: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct InFlight {
    endpoint: String,
    reservation_micros: i64,
    reservation_accounted: bool,
}

impl Checkpoint {
    fn new(handle: String, hard_cap_micros: i64) -> Self {
        Self {
            format: "xunfollow-scan-checkpoint-v1".into(),
            handle,
            hard_cap_micros,
            confirmed_micros: 0,
            uncertain_micros: 0,
            identity: None,
            follower_ids: vec![],
            follower_cursor: String::new(),
            follower_complete: false,
            follower_pages: 0,
            follower_seen_cursors: vec![],
            following_profiles: vec![],
            following_cursor: String::new(),
            following_complete: false,
            following_pages: 0,
            following_seen_cursors: vec![],
            in_flight: None,
            ambiguous_retry_used: false,
        }
    }

    fn reserve(
        &mut self,
        db: &AppDb,
        endpoint: &str,
        reservation_micros: i64,
    ) -> Result<(), AppError> {
        if self
            .confirmed_micros
            .checked_add(self.uncertain_micros)
            .and_then(|value| value.checked_add(reservation_micros))
            .unwrap_or(i64::MAX)
            > self.hard_cap_micros
        {
            return Err(AppError::Budget(format!(
                "下一页的保守预留会超过硬上限 ${}",
                format_micros(self.hard_cap_micros)
            )));
        }
        self.in_flight = Some(InFlight {
            endpoint: endpoint.into(),
            reservation_micros,
            reservation_accounted: false,
        });
        db.save_checkpoint(
            &serde_json::to_string(self).map_err(|error| AppError::Provider(error.to_string()))?,
        )
    }

    fn settle(&mut self, db: &AppDb, actual_micros: i64) -> Result<(), AppError> {
        self.confirmed_micros = self
            .confirmed_micros
            .checked_add(actual_micros)
            .ok_or_else(|| AppError::Budget("费用账本溢出".into()))?;
        self.in_flight = None;
        db.save_checkpoint(
            &serde_json::to_string(self).map_err(|error| AppError::Provider(error.to_string()))?,
        )
    }

    fn mark_uncertain(&mut self, db: &AppDb) -> Result<(), AppError> {
        if let Some(mut in_flight) = self.in_flight.take() {
            if !in_flight.reservation_accounted {
                self.uncertain_micros = self
                    .uncertain_micros
                    .checked_add(in_flight.reservation_micros)
                    .ok_or_else(|| AppError::Budget("不确定费用账本溢出".into()))?;
                in_flight.reservation_accounted = true;
            }
            self.in_flight = Some(in_flight);
        }
        db.save_checkpoint(
            &serde_json::to_string(self).map_err(|error| AppError::Provider(error.to_string()))?,
        )
    }
}

impl TwitterApiIo {
    pub fn new(api_key: String) -> Result<Self, AppError> {
        if api_key.trim().is_empty() || api_key.len() > 1024 {
            return Err(AppError::Validation("API Key 无效".into()));
        }
        let client = Client::builder()
            .https_only(true)
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .map_err(|error| AppError::Network(error.to_string()))?;
        Ok(Self {
            client,
            api_key,
            base_url: BASE_URL.into(),
        })
    }

    #[cfg(test)]
    fn with_test_base_url(base_url: String) -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(5))
                .build()
                .expect("test HTTP client"),
            api_key: "test-key".into(),
            base_url: base_url.trim_end_matches('/').into(),
        }
    }

    pub async fn estimate(
        &self,
        handle: &str,
        hard_cap_micros: i64,
    ) -> Result<CostEstimate, AppError> {
        validate_handle(handle)?;
        if hard_cap_micros < IDENTITY_MICROS {
            return Err(AppError::Budget("硬上限至少应覆盖身份查询费用".into()));
        }
        let account = self.identity(handle).await?;
        let estimate = IDENTITY_MICROS
            + ids_collection_cost(account.followers_count)?
            + profile_collection_cost(account.following_count)?;
        if estimate > hard_cap_micros {
            return Err(AppError::Budget(format!(
                "预计费用 ${} 超过当前硬上限 ${}；请提高上限后再开始。",
                format_micros(estimate),
                format_micros(hard_cap_micros),
            )));
        }
        Ok(CostEstimate {
            estimated_usd: format_micros(estimate),
            hard_cap_usd: format_micros(hard_cap_micros),
            assumption:
                "预估依据当前公开计数；实际按每页 Provider 返回数量结算，超出硬上限会在请求前停止。"
                    .into(),
        })
    }

    pub fn prepare_scan(db: &AppDb, handle: &str, hard_cap_micros: i64) -> Result<(), AppError> {
        validate_handle(handle)?;
        match db.load_checkpoint()? {
            Some(raw) => {
                let checkpoint: Checkpoint = serde_json::from_str(&raw)
                    .map_err(|_| AppError::Provider("本地检查点格式无效".into()))?;
                if checkpoint.handle != handle || checkpoint.hard_cap_micros != hard_cap_micros {
                    return Err(AppError::Validation(
                        "已有未完成任务与当前账号或费用上限不一致".into(),
                    ));
                }
            }
            None => {
                let checkpoint = Checkpoint::new(handle.into(), hard_cap_micros);
                db.save_checkpoint(
                    &serde_json::to_string(&checkpoint)
                        .map_err(|error| AppError::Provider(error.to_string()))?,
                )?;
            }
        }
        Ok(())
    }

    pub async fn scan(
        &self,
        db: &AppDb,
        handle: &str,
        hard_cap_micros: i64,
        acknowledge_ambiguous_retry: bool,
    ) -> Result<(), AppError> {
        validate_handle(handle)?;
        let mut state = match db.load_checkpoint()? {
            Some(raw) => {
                let mut checkpoint: Checkpoint = serde_json::from_str(&raw)
                    .map_err(|_| AppError::Provider("本地检查点格式无效".into()))?;
                if checkpoint.handle != handle || checkpoint.hard_cap_micros != hard_cap_micros {
                    return Err(AppError::Validation(
                        "已有未完成任务与当前账号或费用上限不一致".into(),
                    ));
                }
                if let Some(in_flight) = checkpoint.in_flight.take() {
                    if !acknowledge_ambiguous_retry || checkpoint.ambiguous_retry_used {
                        return Err(AppError::Budget(
                            "上次请求结果或计费不确定，已停止；需要明确批准后才能单次重试".into(),
                        ));
                    }
                    if !in_flight.reservation_accounted {
                        checkpoint.uncertain_micros = checkpoint
                            .uncertain_micros
                            .checked_add(in_flight.reservation_micros)
                            .ok_or_else(|| AppError::Budget("不确定费用账本溢出".into()))?;
                    }
                    checkpoint.ambiguous_retry_used = true;
                    db.save_checkpoint(
                        &serde_json::to_string(&checkpoint)
                            .map_err(|error| AppError::Provider(error.to_string()))?,
                    )?;
                }
                checkpoint
            }
            None => Checkpoint::new(handle.into(), hard_cap_micros),
        };

        if state.identity.is_none() {
            state.reserve(db, "/twitter/user/info", IDENTITY_MICROS)?;
            match self.identity(handle).await {
                Ok(identity) => {
                    state.identity = Some(identity);
                    state.settle(db, IDENTITY_MICROS)?;
                }
                Err(error) => {
                    state.mark_uncertain(db)?;
                    return Err(error);
                }
            }
        }
        let account = state
            .identity
            .clone()
            .ok_or_else(|| AppError::Provider("身份检查点缺失".into()))?;

        while !state.follower_complete {
            if state.follower_pages >= MAX_FOLLOWER_PAGES {
                return Err(AppError::Validation("关注者分页超过安全边界".into()));
            }
            state.reserve(db, "/twitter/user/followers_ids", IDS_RESERVATION_MICROS)?;
            let page = match self
                .follower_ids(&account.username, &state.follower_cursor)
                .await
            {
                Ok(page) => page,
                Err(error) => {
                    state.mark_uncertain(db)?;
                    return Err(error);
                }
            };
            validate_new_ids(&state.follower_ids, &page.ids)?;
            validate_cursor(
                page.has_next,
                &page.next_cursor,
                &state.follower_seen_cursors,
            )?;
            let page_count = page.ids.len();
            state.follower_ids.extend(page.ids);
            if state.follower_ids.len() > MAX_ITEMS {
                return Err(AppError::Validation("关注者数量超过安全边界".into()));
            }
            state.follower_pages += 1;
            state.follower_complete = !page.has_next;
            if page.has_next {
                state.follower_seen_cursors.push(page.next_cursor.clone());
                state.follower_cursor = page.next_cursor;
            }
            state.settle(db, ids_page_cost(page_count)?)?;
        }

        while !state.following_complete {
            if state.following_pages >= MAX_FOLLOWING_PAGES {
                return Err(AppError::Validation("正在关注分页超过安全边界".into()));
            }
            state.reserve(db, "/twitter/user/followings", PROFILE_RESERVATION_MICROS)?;
            let page = match self.followings(handle, &state.following_cursor).await {
                Ok(page) => page,
                Err(error) => {
                    state.mark_uncertain(db)?;
                    return Err(error);
                }
            };
            let known: HashSet<&str> = state
                .following_profiles
                .iter()
                .map(|item| item.stable_x_id.as_str())
                .collect();
            if page
                .profiles
                .iter()
                .any(|item| known.contains(item.stable_x_id.as_str()))
            {
                return Err(AppError::Provider("正在关注分页出现重复 ID".into()));
            }
            validate_cursor(
                page.has_next,
                &page.next_cursor,
                &state.following_seen_cursors,
            )?;
            let page_count = page.profiles.len();
            state.following_profiles.extend(page.profiles);
            if state.following_profiles.len() > MAX_ITEMS {
                return Err(AppError::Validation("正在关注数量超过安全边界".into()));
            }
            state.following_pages += 1;
            state.following_complete = !page.has_next;
            if page.has_next {
                state.following_seen_cursors.push(page.next_cursor.clone());
                state.following_cursor = page.next_cursor;
            }
            state.settle(db, profile_page_cost(page_count)?)?;
            let (account, summary, candidates) = partial_snapshot(&state)?;
            db.replace_snapshot(&account, &summary, &candidates)?;
        }

        let (account, summary, candidates) = complete_snapshot(&state)?;
        db.replace_snapshot(&account, &summary, &candidates)?;
        db.clear_checkpoint()?;
        Ok(())
    }

    async fn identity(&self, handle: &str) -> Result<ResolvedAccount, AppError> {
        let payload = self
            .get_json("/twitter/user/info", &[("userName", handle)])
            .await?;
        let data = payload
            .get("data")
            .and_then(Value::as_object)
            .ok_or_else(|| AppError::Provider("身份响应缺少 data".into()))?;
        let stable_x_id = decimal_id(data.get("id").or_else(|| data.get("id_str")))?;
        let username = handle_value(data.get("userName").or_else(|| data.get("screen_name")))?;
        if !username.eq_ignore_ascii_case(handle) {
            return Err(AppError::Provider("Provider 返回了不同账号".into()));
        }
        Ok(ResolvedAccount {
            stable_x_id,
            username,
            name: bounded_text(data.get("name"), 120),
            followers_count: count(data.get("followers"))?,
            following_count: count(data.get("following"))?,
        })
    }

    async fn follower_ids(&self, handle: &str, cursor_value: &str) -> Result<IdPage, AppError> {
        let mut params = vec![("userName", handle), ("count", "5000")];
        if !cursor_value.is_empty() {
            params.push(("cursor", cursor_value));
        }
        let payload = self
            .get_json("/twitter/user/followers_ids", &params)
            .await?;
        let ids = payload
            .get("ids")
            .and_then(Value::as_array)
            .ok_or_else(|| AppError::Provider("关注者响应缺少 ids".into()))?
            .iter()
            .map(|item| decimal_id(Some(item)))
            .collect::<Result<Vec<_>, _>>()?;
        if ids.len() != ids.iter().collect::<HashSet<_>>().len() {
            return Err(AppError::Provider("关注者分页含重复 ID".into()));
        }
        let (has_next, next_cursor) = cursor(&payload)?;
        Ok(IdPage {
            ids,
            has_next,
            next_cursor,
        })
    }

    async fn followings(&self, handle: &str, cursor_value: &str) -> Result<ProfilePage, AppError> {
        let mut params = vec![("userName", handle), ("pageSize", "200")];
        if !cursor_value.is_empty() {
            params.push(("cursor", cursor_value));
        }
        let payload = self.get_json("/twitter/user/followings", &params).await?;
        let profiles = payload
            .get("followings")
            .and_then(Value::as_array)
            .ok_or_else(|| AppError::Provider("正在关注响应缺少 followings".into()))?
            .iter()
            .map(normalize_profile)
            .collect::<Result<Vec<_>, _>>()?;
        if profiles
            .iter()
            .map(|item| &item.stable_x_id)
            .collect::<HashSet<_>>()
            .len()
            != profiles.len()
        {
            return Err(AppError::Provider("正在关注分页含重复 ID".into()));
        }
        let (has_next, next_cursor) = cursor(&payload)?;
        Ok(ProfilePage {
            profiles,
            has_next,
            next_cursor,
        })
    }

    async fn get_json(&self, path: &str, params: &[(&str, &str)]) -> Result<Value, AppError> {
        let url = format!("{}{path}", self.base_url);
        let response = self
            .client
            .get(url)
            .header("X-API-Key", &self.api_key)
            .query(params)
            .send()
            .await
            .map_err(|error| AppError::Network(error.to_string()))?;
        let status = response.status();
        if status != StatusCode::OK {
            let body = response.text().await.unwrap_or_default();
            let detail = safe_provider_error_message(&body)
                .map(|message| format!("：{message}"))
                .unwrap_or_default();
            return Err(AppError::Network(format!(
                "Provider HTTP {}{detail}",
                status.as_u16()
            )));
        }
        let payload: Value = response
            .json()
            .await
            .map_err(|error| AppError::Provider(error.to_string()))?;
        if payload
            .get("status")
            .and_then(Value::as_str)
            .is_some_and(|status| status != "success")
        {
            return Err(AppError::Provider("Provider 返回失败状态".into()));
        }
        if payload.get("code").is_some_and(|code| {
            code.as_i64()
                .is_some_and(|value| value != 0 && value != 200)
        }) {
            return Err(AppError::Provider("Provider 返回失败代码".into()));
        }
        Ok(payload)
    }
}

fn complete_snapshot(
    state: &Checkpoint,
) -> Result<(Account, ScanSummary, Vec<Candidate>), AppError> {
    if !state.follower_complete || !state.following_complete || state.in_flight.is_some() {
        return Err(AppError::Provider(
            "扫描检查点尚未完成，不能生成名单".into(),
        ));
    }
    snapshot_from_state(state, true)
}

fn partial_snapshot(
    state: &Checkpoint,
) -> Result<(Account, ScanSummary, Vec<Candidate>), AppError> {
    if !state.follower_complete {
        return Err(AppError::Provider("关注者名单尚未读取完成".into()));
    }
    snapshot_from_state(state, false)
}

fn snapshot_from_state(
    state: &Checkpoint,
    complete: bool,
) -> Result<(Account, ScanSummary, Vec<Candidate>), AppError> {
    let identity = state
        .identity
        .as_ref()
        .ok_or_else(|| AppError::Provider("身份检查点缺失".into()))?;
    let following_count = if complete {
        state.following_profiles.len() as i64
    } else {
        identity.following_count
    };
    let maximum_micros = state
        .confirmed_micros
        .checked_add(state.uncertain_micros)
        .ok_or_else(|| AppError::Budget("费用账本溢出".into()))?;
    let followers: HashSet<&str> = state.follower_ids.iter().map(String::as_str).collect();
    let candidates: Vec<Candidate> = state
        .following_profiles
        .iter()
        .cloned()
        .filter(|profile| !followers.contains(profile.stable_x_id.as_str()))
        .collect();
    let summary = ScanSummary {
        captured_at: Some(chrono::Utc::now().to_rfc3339()),
        followers_count: if complete {
            state.follower_ids.len() as i64
        } else {
            identity.followers_count
        },
        following_count,
        non_followback_count: candidates.len() as i64,
        confirmed_cost_usd: format_micros(state.confirmed_micros),
        maximum_possible_cost_usd: format_micros(maximum_micros),
        complete,
    };
    Ok((
        Account {
            username: identity.username.clone(),
            name: Some(identity.name.clone()),
        },
        summary,
        candidates,
    ))
}

struct IdPage {
    ids: Vec<String>,
    has_next: bool,
    next_cursor: String,
}
struct ProfilePage {
    profiles: Vec<Candidate>,
    has_next: bool,
    next_cursor: String,
}

fn validate_handle(handle: &str) -> Result<(), AppError> {
    let cleaned = handle.trim().trim_start_matches('@');
    if cleaned.is_empty()
        || cleaned.len() > 15
        || !cleaned
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
    {
        return Err(AppError::Validation("X 用户名无效".into()));
    }
    Ok(())
}

fn decimal_id(value: Option<&Value>) -> Result<String, AppError> {
    let value = match value {
        Some(Value::String(value)) => value.clone(),
        Some(Value::Number(value)) => value.to_string(),
        _ => String::new(),
    };
    if value.is_empty() || value.len() > 32 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(AppError::Provider("稳定 X ID 无效".into()));
    }
    Ok(value)
}

fn handle_value(value: Option<&Value>) -> Result<String, AppError> {
    let value = value
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .trim_start_matches('@');
    validate_handle(value)?;
    Ok(value.into())
}

fn bounded_text(value: Option<&Value>, maximum: usize) -> String {
    value
        .and_then(Value::as_str)
        .unwrap_or("")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(maximum)
        .collect()
}
fn count(value: Option<&Value>) -> Result<i64, AppError> {
    value
        .and_then(Value::as_i64)
        .filter(|value| *value >= 0)
        .ok_or_else(|| AppError::Provider("计数无效".into()))
}

fn cursor(payload: &Value) -> Result<(bool, String), AppError> {
    let has_next = payload
        .get("has_next_page")
        .or_else(|| payload.get("hasNextPage"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let next = payload
        .get("next_cursor")
        .or_else(|| payload.get("nextCursor"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let next = if next == "0" { "" } else { next };
    if next.len() > 2048 || (has_next && next.is_empty()) {
        return Err(AppError::Provider("分页游标无效".into()));
    }
    Ok((has_next, next.into()))
}

fn normalize_profile(value: &Value) -> Result<Candidate, AppError> {
    let item = value
        .as_object()
        .ok_or_else(|| AppError::Provider("用户资料无效".into()))?;
    let stable_x_id = decimal_id(item.get("id").or_else(|| item.get("id_str")))?;
    let username = handle_value(
        item.get("userName")
            .or_else(|| item.get("username"))
            .or_else(|| item.get("screen_name")),
    )?;
    let image = item
        .get("profilePicture")
        .or_else(|| item.get("profile_image_url_https"))
        .and_then(Value::as_str)
        .filter(|value| {
            value.starts_with("https://pbs.twimg.com/")
                || value.starts_with("https://abs.twimg.com/")
        })
        .filter(|value| value.len() <= 500)
        .map(str::to_owned);
    Ok(Candidate {
        stable_x_id: stable_x_id.clone(),
        username: username.clone(),
        name: bounded_text(item.get("name").or_else(|| item.get("displayName")), 120),
        profile_image_url: image,
        followers_count: count(
            item.get("followers")
                .or_else(|| item.get("followersCount"))
                .or_else(|| item.get("followers_count")),
        )?,
        following_count: count(
            item.get("following")
                .or_else(|| item.get("followingCount"))
                .or_else(|| item.get("following_count")),
        )?,
        x_url: format!("https://x.com/{username}"),
    })
}

fn validate_new_ids(existing: &[String], page: &[String]) -> Result<(), AppError> {
    let known: HashSet<&str> = existing.iter().map(String::as_str).collect();
    if page.iter().any(|item| known.contains(item.as_str())) {
        return Err(AppError::Provider("关注者分页出现重复 ID".into()));
    }
    Ok(())
}

fn validate_cursor(has_next: bool, cursor: &str, seen: &[String]) -> Result<(), AppError> {
    if has_next && seen.iter().any(|item| item == cursor) {
        return Err(AppError::Provider("分页游标重复".into()));
    }
    Ok(())
}

fn ids_page_cost(count: usize) -> Result<i64, AppError> {
    let value = if count >= 4000 {
        ((count as i64) * 9 + 1) / 2
    } else if count >= 200 {
        (count as i64) * 10
    } else {
        (count as i64) * 20
    };
    Ok(value.max(1000))
}
fn profile_page_cost(count: usize) -> Result<i64, AppError> {
    let value = if count >= 200 {
        (count as i64) * 10
    } else if count >= 100 {
        (count as i64) * 20
    } else {
        (count as i64) * 30
    };
    Ok(value.max(600))
}
fn ids_collection_cost(count: i64) -> Result<i64, AppError> {
    paged_cost(count, 5000, ids_page_cost)
}
fn profile_collection_cost(count: i64) -> Result<i64, AppError> {
    paged_cost(count, 200, profile_page_cost)
}
fn paged_cost(
    count: i64,
    page_size: i64,
    cost: fn(usize) -> Result<i64, AppError>,
) -> Result<i64, AppError> {
    if count < 0 {
        return Err(AppError::Provider("公开计数无效".into()));
    }
    let full = count / page_size;
    let tail = count % page_size;
    let mut total = full
        .checked_mul(cost(page_size as usize)?)
        .ok_or_else(|| AppError::Budget("预估费用溢出".into()))?;
    if tail > 0 {
        total = total
            .checked_add(cost(tail as usize)?)
            .ok_or_else(|| AppError::Budget("预估费用溢出".into()))?;
    }
    Ok(total)
}

fn format_micros(micros: i64) -> String {
    format!("{}.{:06}", micros / 1_000_000, micros.rem_euclid(1_000_000))
}

fn safe_provider_error_message(body: &str) -> Option<String> {
    let payload: Value = serde_json::from_str(body).ok()?;
    let message = ["message", "msg", "detail", "error"]
        .iter()
        .find_map(|key| payload.get(*key).and_then(Value::as_str))?;
    let cleaned: String = message
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .take(180)
        .collect();
    (!cleaned.trim().is_empty()).then(|| cleaned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn checkpoint_db() -> (AppDb, std::path::PathBuf) {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("xunfollow-provider-test-{nonce}"));
        let db = AppDb::new(directory.join("test.sqlite3")).unwrap();
        (db, directory)
    }
    #[test]
    fn bulk_id_pricing_is_conservative() {
        assert_eq!(ids_page_cost(5000).unwrap(), 22_500);
        assert_eq!(ids_page_cost(4563).unwrap(), 20_534);
        assert_eq!(profile_page_cost(200).unwrap(), 2_000);
    }

    #[test]
    fn provider_error_detail_is_bounded_and_optional() {
        assert_eq!(
            safe_provider_error_message(r#"{"message":"invalid API key"}"#).as_deref(),
            Some("invalid API key")
        );
        assert!(safe_provider_error_message("not json").is_none());
    }
    #[test]
    fn rejects_invalid_handle() {
        assert!(validate_handle("invalid-handle!").is_err());
    }

    #[test]
    fn uncertain_request_is_conservatively_accounted_and_persisted() {
        let (db, directory) = checkpoint_db();
        let mut checkpoint = Checkpoint::new("fixture".into(), 100_000);
        checkpoint
            .reserve(&db, "/twitter/user/followings", 4_000)
            .unwrap();
        checkpoint.mark_uncertain(&db).unwrap();
        assert_eq!(checkpoint.confirmed_micros, 0);
        assert_eq!(checkpoint.uncertain_micros, 4_000);
        assert!(checkpoint.in_flight.as_ref().unwrap().reservation_accounted);
        assert!(db.load_checkpoint().unwrap().is_some());
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn reserve_refuses_to_cross_hard_cap_before_request() {
        let (db, directory) = checkpoint_db();
        let mut checkpoint = Checkpoint::new("fixture".into(), 1_000);
        assert!(checkpoint
            .reserve(&db, "/twitter/user/info", 1_001)
            .is_err());
        assert!(db.load_checkpoint().unwrap().is_none());
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn deidentified_provider_fixture_produces_only_non_followbacks() {
        let mut state = Checkpoint::new("fixture".into(), 100_000);
        state.identity = Some(ResolvedAccount {
            stable_x_id: "1".into(),
            username: "fixture".into(),
            name: "Fixture Account".into(),
            followers_count: 2,
            following_count: 3,
        });
        state.follower_ids = vec!["100".into(), "101".into()];
        state.follower_complete = true;
        state.following_profiles = vec![
            Candidate {
                stable_x_id: "100".into(),
                username: "mutual".into(),
                name: "Mutual".into(),
                profile_image_url: None,
                followers_count: 1,
                following_count: 1,
                x_url: "https://x.com/mutual".into(),
            },
            Candidate {
                stable_x_id: "200".into(),
                username: "cleanup_one".into(),
                name: "Cleanup One".into(),
                profile_image_url: None,
                followers_count: 1,
                following_count: 1,
                x_url: "https://x.com/cleanup_one".into(),
            },
            Candidate {
                stable_x_id: "300".into(),
                username: "cleanup_two".into(),
                name: "Cleanup Two".into(),
                profile_image_url: None,
                followers_count: 1,
                following_count: 1,
                x_url: "https://x.com/cleanup_two".into(),
            },
        ];
        state.following_complete = true;
        state.confirmed_micros = 50_000;
        state.uncertain_micros = 4_000;

        let (account, summary, candidates) = complete_snapshot(&state).unwrap();
        assert_eq!(account.username, "fixture");
        assert_eq!(summary.followers_count, 2);
        assert_eq!(summary.following_count, 3);
        assert_eq!(summary.non_followback_count, 2);
        assert_eq!(summary.confirmed_cost_usd, "0.050000");
        assert_eq!(summary.maximum_possible_cost_usd, "0.054000");
        assert_eq!(
            candidates
                .iter()
                .map(|item| item.stable_x_id.as_str())
                .collect::<Vec<_>>(),
            vec!["200", "300"]
        );
    }

    #[test]
    fn partial_snapshot_exposes_actionable_candidates_before_scan_completes() {
        let mut state = Checkpoint::new("fixture".into(), 100_000);
        state.identity = Some(ResolvedAccount {
            stable_x_id: "1".into(),
            username: "fixture".into(),
            name: "Fixture Account".into(),
            followers_count: 2,
            following_count: 3,
        });
        state.follower_ids = vec!["100".into(), "101".into()];
        state.follower_complete = true;
        state.following_profiles = vec![Candidate {
            stable_x_id: "200".into(),
            username: "cleanup_one".into(),
            name: "Cleanup One".into(),
            profile_image_url: None,
            followers_count: 1,
            following_count: 1,
            x_url: "https://x.com/cleanup_one".into(),
        }];

        let (_, summary, candidates) = partial_snapshot(&state).unwrap();
        assert!(!summary.complete);
        assert_eq!(summary.following_count, 3);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].username, "cleanup_one");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn mock_provider_scan_persists_a_deidentified_snapshot_end_to_end() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            for _ in 0..3 {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = [0_u8; 4096];
                let bytes = stream.read(&mut request).unwrap();
                let request = String::from_utf8_lossy(&request[..bytes]);
                let body = if request.contains("/twitter/user/info") {
                    r#"{"status":"success","data":{"id":"1","userName":"fixture","name":"Fixture Account","followers":2,"following":3}}"#
                } else if request.contains("/twitter/user/followers_ids") {
                    r#"{"status":"success","ids":["100","101"],"has_next_page":false}"#
                } else if request.contains("/twitter/user/followings") {
                    r#"{"status":"success","followings":[{"id":"100","userName":"mutual","name":"Mutual","followers":1,"following":1},{"id":"200","userName":"cleanup_one","name":"Cleanup One","followers":1,"following":1},{"id":"300","userName":"cleanup_two","name":"Cleanup Two","followers":1,"following":1}],"has_next_page":false}"#
                } else {
                    panic!("unexpected mock Provider request: {request}");
                };
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                )
                .unwrap();
            }
        });

        let (db, directory) = checkpoint_db();
        let provider = TwitterApiIo::with_test_base_url(format!("http://{address}"));
        provider.scan(&db, "fixture", 100_000, false).await.unwrap();
        server.join().unwrap();

        let snapshot = db.bootstrap().unwrap();
        assert_eq!(snapshot.account.unwrap().username, "fixture");
        assert_eq!(snapshot.summary.unwrap().non_followback_count, 2);
        assert_eq!(
            snapshot
                .candidates
                .iter()
                .map(|item| item.username.as_str())
                .collect::<Vec<_>>(),
            vec!["cleanup_one", "cleanup_two"]
        );
        assert!(db.load_checkpoint().unwrap().is_none());
        std::fs::remove_dir_all(directory).unwrap();
    }
}
