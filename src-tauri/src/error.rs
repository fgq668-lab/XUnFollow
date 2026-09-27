use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("本地数据库错误：{0}")]
    Database(#[from] rusqlite::Error),
    #[error("网络请求失败：{0}")]
    Network(String),
    #[error("Provider 响应无效：{0}")]
    Provider(String),
    #[error("输入无效：{0}")]
    Validation(String),
    #[error("费用上限保护：{0}")]
    Budget(String),
    #[error("外部链接被拒绝")]
    ExternalUrlRejected,
}

impl serde::Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}
