use anyhow::{Context, Result};
use dirs::config_dir;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const APP_NAME: &str = "orquestra";
const CONFIG_FILE: &str = "config.toml";
const DEFAULT_API_BASE: &str = "https://api.orquestra.dev";
const DEFAULT_RPC: &str = "https://api.mainnet-beta.solana.com";

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct Config {
    pub project_id: Option<String>,
    pub api_key: Option<String>,
    pub rpc_url: Option<String>,
    pub keypair_path: Option<String>,
    pub api_base_url: Option<String>,    /// Path to a local Solana IDL JSON file (enables offline/file mode)
    pub idl_path:     Option<String>,}

impl Config {
    pub fn load() -> Result<Self> {
        let path = config_path()?;
        if !path.exists() {
            return Ok(Config::default());
        }
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read config at {}", path.display()))?;
        let config: Config = toml::from_str(&content)
            .with_context(|| "Failed to parse config.toml")?;
        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let path = config_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create config dir {}", parent.display()))?;
        }
        let content = toml::to_string_pretty(self)?;
        std::fs::write(&path, content)
            .with_context(|| format!("Failed to write config at {}", path.display()))?;
        Ok(())
    }

    /// Merge non-None fields from another Config into self
    pub fn merge(&mut self, other: Config) {
        if other.project_id.is_some() {
            self.project_id = normalize_optional(other.project_id);
        }
        if other.api_key.is_some() {
            self.api_key = normalize_optional(other.api_key);
        }
        if other.rpc_url.is_some() {
            self.rpc_url = normalize_optional(other.rpc_url);
        }
        if other.keypair_path.is_some() {
            self.keypair_path = normalize_optional(other.keypair_path);
        }
        if other.api_base_url.is_some() {
            self.api_base_url = normalize_optional(other.api_base_url);
        }
        if other.idl_path.is_some() {
            self.idl_path = normalize_optional(other.idl_path);
        }
    }

    pub fn api_base(&self) -> &str {
        self.api_base_url
            .as_deref()
            .unwrap_or(DEFAULT_API_BASE)
    }

    pub fn rpc(&self) -> &str {
        self.rpc_url.as_deref().unwrap_or(DEFAULT_RPC)
    }

    /// Show config, masking the API key
    pub fn display(&self) -> String {
        let masked_key = self.api_key.as_deref().map(mask_secret).unwrap_or_else(|| "(not set)".to_string());
        format!(
            "program_id  : {}\napi_key     : {}\nrpc_url     : {}\nkeypair_path: {}\napi_base_url: {}\nidl_path    : {}",
            self.project_id.as_deref().unwrap_or("(not set)"),
            masked_key,
            self.rpc_url.as_deref().unwrap_or(DEFAULT_RPC),
            self.keypair_path.as_deref().unwrap_or("(not set)"),
            self.api_base_url.as_deref().unwrap_or(DEFAULT_API_BASE),
            self.idl_path.as_deref().unwrap_or("(not set)"),
        )
    }

    pub fn require_project_id(&self) -> Result<&str> {
        self.project_id.as_deref()
            .with_context(|| "project_id not set — run: orquestra config set --project-id <id>")
    }

    pub fn optional_api_key(&self) -> Option<&str> {
        self.api_key.as_deref()
    }
}

fn config_path() -> Result<PathBuf> {
    let dir = config_dir()
        .with_context(|| "Cannot determine config directory")?;
    Ok(dir.join(APP_NAME).join(CONFIG_FILE))
}

fn mask_secret(s: &str) -> String {
    let len = s.len();
    if len <= 8 {
        return "*".repeat(len);
    }
    format!("{}***{}", &s[..4], &s[len - 4..])
}

fn normalize_optional(value: Option<String>) -> Option<String> {
    value.and_then(|s| {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}
