use async_trait::async_trait;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProviderInfo {
    pub name: String,
    pub base_url: String,
}

#[async_trait]
pub trait LLMProvider: Send + Sync {
    async fn load(&mut self, active: &modelops::ActiveModel) -> anyhow::Result<()>;
    async fn complete(&self, prompt: &str) -> anyhow::Result<String>;
    async fn ping(&self) -> anyhow::Result<()>;
    fn info(&self) -> ProviderInfo;
}
