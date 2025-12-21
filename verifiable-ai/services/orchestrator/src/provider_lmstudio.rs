use async_trait::async_trait;
use anyhow::Context;

pub struct LmStudioProvider {
    base_url: String,
    current_model: Option<String>,
    client: reqwest::Client,
}

impl LmStudioProvider {
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            current_model: None,
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl crate::provider::LLMProvider for LmStudioProvider {
    async fn load(&mut self, active: &modelops::ActiveModel) -> anyhow::Result<()> {
        // Minimal: map active model repoid -> LM Studio model ID
        // Often these won't match exactly without a mapping, but for now we use repo_id.
        self.current_model = Some(active.repo_id.clone());
        Ok(())
    }

    async fn complete(&self, prompt: &str) -> anyhow::Result<String> {
        let model = self.current_model.clone().unwrap_or_else(|| "default".to_string());

        let body = serde_json::json!({
            "model": model,
            "messages": [{"role":"user","content": prompt}],
            "temperature": 0.2
        });

        let url = format!("{}/v1/chat/completions", self.base_url);
        let resp = self.client.post(url).json(&body).send().await?.error_for_status()?;
        let json: serde_json::Value = resp.json().await?;

        // Extract content
        Ok(json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string())
    }

    async fn ping(&self) -> anyhow::Result<()> {
        let url = format!("{}/v1/models", self.base_url);
        self.client.get(url).send().await?.error_for_status()?;
        Ok(())
    }

    fn info(&self) -> crate::provider::ProviderInfo {
        crate::provider::ProviderInfo {
            name: "lmstudio".to_string(),
            base_url: self.base_url.clone(),
        }
    }
}
