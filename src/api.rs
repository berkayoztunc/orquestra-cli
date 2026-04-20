use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Minimal type representation from the IDL
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum IdlType {
    Simple(String),
    Complex(serde_json::Value),
}

impl std::fmt::Display for IdlType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IdlType::Simple(s) => write!(f, "{s}"),
            IdlType::Complex(v) => write!(f, "{v}"),
        }
    }
}

impl Default for IdlType {
    fn default() -> Self {
        IdlType::Simple("unknown".to_string())
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct InstructionArg {
    pub name: String,
    #[serde(rename = "type", default)]
    pub ty: IdlType,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct InstructionAccount {
    pub name: String,
    #[serde(rename = "isMut", default)]
    pub is_mut: bool,
    #[serde(rename = "isSigner", default)]
    pub is_signer: bool,
    #[serde(rename = "isOptional", default)]
    pub is_optional: bool,
    pub pda: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Instruction {
    pub name: String,
    #[serde(default)]
    pub docs: Vec<String>,
    #[serde(default)]
    pub args: Vec<InstructionArg>,
    #[serde(default)]
    pub accounts: Vec<InstructionAccount>,
}

/// Project lookup: GET /api/projects/by-program/{programAddress}
#[derive(Debug, Deserialize)]
pub struct ProjectByProgramResponse {
    pub project: ProjectInfo,
}

#[derive(Debug, Deserialize)]
pub struct ProjectInfo {
    pub id: String,
    pub name: String,
    #[allow(dead_code)]
    pub program_id: String,
}

/// Response from GET /api/{projectId}/instructions
#[derive(Debug, Deserialize)]
pub struct InstructionsListResponse {
    #[serde(default)]
    pub instructions: Vec<Instruction>,
    // Some responses wrap differently; handle both
    #[serde(default)]
    pub data: Option<Vec<Instruction>>,
}

impl InstructionsListResponse {
    pub fn into_list(self) -> Vec<Instruction> {
        if !self.instructions.is_empty() {
            self.instructions
        } else {
            self.data.unwrap_or_default()
        }
    }
}

/// Response from GET /api/programs/{id}/instructions/{name}
#[derive(Debug, Deserialize)]
pub struct InstructionDetailResponse {
    pub instruction: Option<Instruction>,
    // fallback: the instruction itself is the root
    #[serde(flatten)]
    pub root: Option<Instruction>,
}

impl InstructionDetailResponse {
    pub fn into_instruction(self) -> Option<Instruction> {
        self.instruction.or(self.root)
    }
}

/// PDA list: GET /api/{id}/pda
#[derive(Debug, Deserialize)]
pub struct PdaListResponse {
    #[serde(rename = "pdaAccounts")]
    pub pda_accounts: Vec<PdaAccount>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PdaAccount {
    pub instruction: String,
    pub account: String,
    #[serde(default)]
    pub seeds: Vec<PdaSeed>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PdaSeed {
    pub kind: String,
    /// Constant label (present on "const" seeds)
    #[allow(dead_code)]
    pub description: Option<String>,
    /// Argument name (present on "arg" seeds)
    pub name: Option<String>,
    /// Argument type hint, e.g. "string", "u64"
    #[serde(rename = "type")]
    pub ty: Option<String>,
}

/// POST /api/{id}/pda/derive — request body
#[derive(Debug, Serialize)]
pub struct DeriveRequest {
    pub instruction: String,
    pub account: String,
    #[serde(rename = "seedValues")]
    pub seed_values: HashMap<String, String>,
}

/// POST /api/{id}/pda/derive — response
#[derive(Debug, Deserialize)]
pub struct DeriveResponse {
    pub pda: String,
    pub bump: u8,
    #[serde(rename = "programId")]
    pub program_id: String,
    pub seeds: Vec<DerivedSeed>,
}

#[derive(Debug, Deserialize)]
pub struct DerivedSeed {
    pub kind: String,
    pub description: Option<String>,
    pub name: Option<String>,
    pub value: Option<String>,
    pub hex: String,
}

/// Search: GET /api/projects/search?q={query}
#[derive(Debug, Deserialize)]
pub struct SearchProject {
    #[allow(dead_code)]
    pub id: String,
    pub name: String,
    #[serde(rename = "program_id")]
    pub program_id: String,
    #[allow(dead_code)]
    pub description: Option<String>,
    pub category: Option<String>,
    pub tags: Option<String>,
    #[allow(dead_code)]
    pub username: Option<String>,
    #[allow(dead_code)]
    pub match_type: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct SearchPagination {
    pub total: u64,
    pub page: u64,
    #[serde(rename = "totalPages")]
    pub total_pages: u64,
}

#[derive(Debug, Deserialize)]
pub struct SearchResponse {
    pub projects: Vec<SearchProject>,
    pub pagination: Option<SearchPagination>,
}

/// POST body for /build
#[derive(Debug, Serialize)]
pub struct BuildRequest {
    pub accounts: HashMap<String, String>,
    pub args: HashMap<String, serde_json::Value>,
    #[serde(rename = "feePayer")]
    pub fee_payer: String,
    pub network: String,
}

/// Response from POST .../build
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct BuildResponse {
    pub transaction: String,
    #[serde(rename = "serializedTransaction")]
    pub serialized_transaction: Option<String>,
    pub message: Option<String>,
    #[serde(rename = "estimatedFee")]
    pub estimated_fee: Option<u64>,
}

pub struct ApiClient {
    client: reqwest::Client,
    base_url: String,
    api_key: Option<String>,
}

impl ApiClient {
    pub fn new(base_url: impl Into<String>, api_key: Option<impl Into<String>>) -> Self {
        let client = reqwest::Client::builder()
            .user_agent(concat!("orquestra-cli/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("Failed to build HTTP client");
        Self {
            client,
            base_url: base_url.into().trim_end_matches('/').to_string(),
            api_key: api_key.map(|k| k.into()),
        }
    }

    fn apply_api_key(&self, req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match &self.api_key {
            Some(key) => req.header("X-API-Key", key),
            None => req,
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}/{}", self.base_url, path.trim_start_matches('/'))
    }

    /// Resolve a Solana program address to the orquestra internal project ID.
    /// Calls GET /api/projects/by-program/{program_address}
    pub async fn resolve_project_id(&self, program_address: &str) -> Result<ProjectInfo> {
        let url = self.url(&format!("api/projects/by-program/{program_address}"));
        let resp = self
            .apply_api_key(self.client.get(&url))
            .send()
            .await
            .with_context(|| format!("Failed to reach {url}"))?;

        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            bail!("Could not find program '{program_address}' on orquestra: {status}: {text}");
        }

        let parsed: ProjectByProgramResponse = serde_json::from_str(&text)
            .with_context(|| format!("Cannot parse by-program response:\n{text}"))?;
        Ok(parsed.project)
    }

    pub async fn list_instructions(&self, project_id: &str) -> Result<Vec<Instruction>> {
        let url = self.url(&format!("api/{project_id}/instructions"));
        let resp = self
            .apply_api_key(self.client.get(&url))
            .send()
            .await
            .with_context(|| format!("Failed to reach {url}"))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            bail!("API error {status}: {body}");
        }

        // Try structured parse first, fall back to raw array
        let text = resp.text().await?;
        if let Ok(list_resp) = serde_json::from_str::<InstructionsListResponse>(&text) {
            let list = list_resp.into_list();
            if !list.is_empty() {
                return Ok(list);
            }
        }
        // Try direct array
        let list: Vec<Instruction> = serde_json::from_str(&text)
            .with_context(|| format!("Cannot parse instructions response:\n{text}"))?;
        Ok(list)
    }

    pub async fn get_instruction(&self, project_id: &str, name: &str) -> Result<Instruction> {
        let url = self.url(&format!("api/{project_id}/instructions/{name}"));
        let resp = self
            .apply_api_key(self.client.get(&url))
            .send()
            .await
            .with_context(|| format!("Failed to reach {url}"))?;

        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            bail!("API error {status}: {text}");
        }

        // Try wrapped {"instruction": ...} first, then direct
        if let Ok(detail) = serde_json::from_str::<InstructionDetailResponse>(&text) {
            if let Some(ix) = detail.into_instruction() {
                return Ok(ix);
            }
        }
        let ix: Instruction = serde_json::from_str(&text)
            .with_context(|| format!("Cannot parse instruction response:\n{text}"))?;
        Ok(ix)
    }

    pub async fn build_transaction(
        &self,
        project_id: &str,
        instruction_name: &str,
        accounts: HashMap<String, String>,
        args: HashMap<String, serde_json::Value>,
        fee_payer: String,
        network: &str,
    ) -> Result<BuildResponse> {
        let url = self.url(&format!(
            "api/{project_id}/instructions/{instruction_name}/build"
        ));
        let body = BuildRequest {
            accounts,
            args,
            fee_payer,
            network: network.to_string(),
        };
        let resp = self
            .apply_api_key(self.client.post(&url))
            .json(&body)
            .send()
            .await
            .with_context(|| format!("Failed to reach {url}"))?;

        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            bail!("API error {status}: {text}");
        }

        let build: BuildResponse = serde_json::from_str(&text)
            .with_context(|| format!("Cannot parse build response:\n{text}"))?;
        Ok(build)
    }

    /// List all PDA accounts for a project.
    /// Calls GET /api/{id}/pda
    pub async fn list_pdas(&self, project_id: &str) -> Result<Vec<PdaAccount>> {
        let url = self.url(&format!("api/{project_id}/pda"));
        let resp = self
            .apply_api_key(self.client.get(&url))
            .send()
            .await
            .with_context(|| format!("Failed to reach {url}"))?;

        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            bail!("API error {status}: {text}");
        }

        let parsed: PdaListResponse = serde_json::from_str(&text)
            .with_context(|| format!("Cannot parse PDA list response:\n{text}"))?;
        Ok(parsed.pda_accounts)
    }

    /// Derive a PDA address.
    /// Calls POST /api/{id}/pda/derive
    pub async fn derive_pda(
        &self,
        project_id: &str,
        instruction: &str,
        account: &str,
        _pda_seeds: &[crate::api::PdaSeed],
        arg_values: HashMap<String, String>,
    ) -> Result<DeriveResponse> {
        let url = self.url(&format!("api/{project_id}/pda/derive"));
        let body = DeriveRequest {
            instruction: instruction.to_string(),
            account: account.to_string(),
            seed_values: arg_values,
        };
        let resp = self
            .apply_api_key(self.client.post(&url))
            .json(&body)
            .send()
            .await
            .with_context(|| format!("Failed to reach {url}"))?;

        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            bail!("API error {status}: {text}");
        }

        let derived: DeriveResponse = serde_json::from_str(&text)
            .with_context(|| format!("Cannot parse derive response:\n{text}"))?;
        Ok(derived)
    }

    /// Search programs by name or description.
    /// Calls GET /api/projects?search={query}&page={page}
    pub async fn search_programs(&self, query: &str, page: u64) -> Result<SearchResponse> {
        let page_str = page.to_string();
        let url = self.url("api/projects");
        let resp = self
            .apply_api_key(
                self.client
                    .get(&url)
                    .query(&[("search", query), ("page", page_str.as_str())])
                    .header("Cache-Control", "no-cache")
                    .header("Pragma", "no-cache"),
            )
            .send()
            .await
            .with_context(|| format!("Failed to reach {url}"))?;

        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            bail!("API error {status}: {text}");
        }

        // Try wrapped {"projects": [...]} first, then direct array
        if let Ok(sr) = serde_json::from_str::<SearchResponse>(&text) {
            return Ok(sr);
        }
        let list: Vec<SearchProject> = serde_json::from_str(&text)
            .with_context(|| format!("Cannot parse search response:\n{text}"))?;
        Ok(SearchResponse { projects: list, pagination: None })
    }

    /// Fetch the raw IDL JSON for a project.
    /// Calls GET /api/{projectId}/idl
    pub async fn fetch_idl(&self, project_id: &str) -> Result<String> {
        let url = self.url(&format!("api/{project_id}/idl"));
        let resp = self
            .apply_api_key(self.client.get(&url))
            .send()
            .await
            .with_context(|| format!("Failed to reach {url}"))?;

        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            bail!("API error {status} fetching IDL for project '{project_id}': {text}");
        }

        Ok(text)
    }
}
