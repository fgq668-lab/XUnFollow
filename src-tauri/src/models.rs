use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub stable_x_id: String,
    pub username: String,
    pub name: String,
    pub profile_image_url: Option<String>,
    pub followers_count: i64,
    pub following_count: i64,
    pub x_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Decision {
    pub status: DecisionStatus,
    pub action_day: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub stable_x_id: String,
    pub username: String,
    pub name: String,
    pub status: DecisionStatus,
    pub action_day: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportedDecision {
    pub status: DecisionStatus,
    pub action_day: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DecisionStatus {
    Unfollowed,
    Keep,
    Later,
    Changed,
}

impl DecisionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Unfollowed => "unfollowed",
            Self::Keep => "keep",
            Self::Later => "later",
            Self::Changed => "changed",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "unfollowed" => Some(Self::Unfollowed),
            "keep" => Some(Self::Keep),
            "later" => Some(Self::Later),
            "changed" => Some(Self::Changed),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub username: String,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanSummary {
    pub captured_at: Option<String>,
    pub followers_count: i64,
    pub following_count: i64,
    pub non_followback_count: i64,
    pub confirmed_cost_usd: String,
    pub maximum_possible_cost_usd: String,
    pub complete: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bootstrap {
    pub account: Option<Account>,
    pub candidates: Vec<Candidate>,
    pub decisions: std::collections::BTreeMap<String, Decision>,
    pub history: Vec<HistoryEntry>,
    pub summary: Option<ScanSummary>,
    pub daily_goal: i64,
    pub batch_size: i64,
    pub api_key_configured: bool,
    pub pending_scan: Option<PendingScan>,
}

/// Metadata only: the API key and the Provider payload never leave the Rust core.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingScan {
    pub handle: String,
    pub hard_cap_usd: String,
    pub needs_explicit_retry: bool,
    pub follower_pages: usize,
    pub following_pages: usize,
    pub follower_ids_loaded: usize,
    pub following_profiles_loaded: usize,
    pub followers_total: i64,
    pub following_total: i64,
    pub follower_complete: bool,
    pub following_complete: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CostEstimate {
    pub estimated_usd: String,
    pub hard_cap_usd: String,
    pub assumption: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedAccount {
    pub stable_x_id: String,
    pub username: String,
    pub name: String,
    pub followers_count: i64,
    pub following_count: i64,
}
