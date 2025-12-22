use anyhow::{bail, Context, Result};

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub comfyui_url: String,
    pub database_url: String,

    pub s3_endpoint: String,
    pub s3_region: String,
    pub s3_bucket: String,
    pub s3_access_key: String,
    pub s3_secret_key: String,
    pub s3_force_path_style: bool,
    pub bind_addr: String,
}

impl AppConfig {
    pub fn from_env() -> Result<Self> {
        let comfyui_url = get("COMFYUI_URL")?;
        let database_url = get("DATABASE_URL")?;

        let s3_endpoint = get("S3_ENDPOINT")?;
        let s3_region = std::env::var("S3_REGION").unwrap_or_else(|_| "us-east-1".to_string());
        let s3_bucket = get("S3_BUCKET")?;
        let s3_access_key = get("S3_ACCESS_KEY")?;
        let s3_secret_key = get("S3_SECRET_KEY")?;
        let s3_force_path_style = std::env::var("S3_FORCE_PATH_STYLE")
            .ok()
            .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
            .unwrap_or(true);
            
        let bind_addr = std::env::var("ORCH_BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string());

        // Tiny sanity checks (fail fast, fail loud)
        if !comfyui_url.starts_with("http://") && !comfyui_url.starts_with("https://") {
            bail!("COMFYUI_URL must start with http:// or https://");
        }
        if !s3_endpoint.starts_with("http://") && !s3_endpoint.starts_with("https://") {
            bail!("S3_ENDPOINT must start with http:// or https://");
        }

        Ok(Self {
            comfyui_url,
            database_url,
            s3_endpoint,
            s3_region,
            s3_bucket,
            s3_access_key,
            s3_secret_key,
            s3_force_path_style,
            bind_addr,
        })
    }
}

fn get(key: &str) -> Result<String> {
    std::env::var(key).with_context(|| format!("Missing required env var: {key}"))
}
