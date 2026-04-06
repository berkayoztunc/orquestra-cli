//! Local Solana IDL JSON parsing and transaction building.
//!
//! Supports the native Anchor/Solana IDL format (as in testid.json).
//! When `config.idl_path` is set the CLI operates in "file mode" and never
//! contacts the Orquestra API.

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

use crate::api::{IdlType, Instruction, InstructionAccount, InstructionArg, PdaAccount, PdaSeed};

// ── IDL struct definitions ────────────────────────────────────────────────────

/// Top-level Solana/Anchor IDL file (native JSON format).
#[derive(Debug, Deserialize, Clone)]
pub struct SolanaIdl {
    pub address: String,
    #[serde(default)]
    pub instructions: Vec<IdlInstruction>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct IdlInstruction {
    pub name: String,
    #[serde(default)]
    pub discriminator: Vec<u8>,
    #[serde(default)]
    pub accounts: Vec<IdlAccount>,
    #[serde(default)]
    pub args: Vec<IdlArg>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct IdlAccount {
    pub name: String,
    #[serde(default)]
    pub writable: bool,
    #[serde(default)]
    pub signer: bool,
    /// Hardcoded program address (e.g. system_program, token_program).
    pub address: Option<String>,
    /// PDA definition — when present this account can be auto-derived.
    pub pda: Option<IdlPda>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct IdlPda {
    pub seeds: Vec<IdlSeed>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct IdlSeed {
    /// "const" | "arg" | "account"
    pub kind: String,
    /// Raw byte array for `const` seeds.
    pub value: Option<Vec<u8>>,
    /// Arg name or account name for `arg` / `account` seeds.
    pub path: Option<String>,
    /// Optional program override (e.g. ATA derivation) — we carry it but
    /// don't use it for derivation; ATA seeds are handled via "account" kind.
    #[allow(dead_code)]
    pub program: Option<Value>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct IdlArg {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: Value,
}

// ── Parse ─────────────────────────────────────────────────────────────────────

/// Read and parse a Solana IDL JSON file from disk.
pub fn parse_idl_file(path: &str) -> Result<SolanaIdl> {
    let content = std::fs::read_to_string(path.trim())
        .with_context(|| format!("Cannot read IDL file: {path}"))?;
    let idl: SolanaIdl = serde_json::from_str(&content)
        .with_context(|| format!("IDL file is not valid JSON or missing required fields: {path}"))?;
    Ok(idl)
}

// ── Conversions to API-compat types ──────────────────────────────────────────

/// Convert an IDL to the `api::Instruction` vec used by existing interactive
/// prompts (`collect_args`, `collect_accounts`, etc.).
pub fn idl_to_instructions(idl: &SolanaIdl) -> Vec<Instruction> {
    idl.instructions
        .iter()
        .map(|ix| Instruction {
            name: ix.name.clone(),
            docs: vec![],
            args: ix
                .args
                .iter()
                .map(|a| InstructionArg {
                    name: a.name.clone(),
                    ty: match &a.ty {
                        Value::String(s) => IdlType::Simple(s.clone()),
                        v => IdlType::Complex(v.clone()),
                    },
                })
                .collect(),
            accounts: ix
                .accounts
                .iter()
                .map(|a| InstructionAccount {
                    name: a.name.clone(),
                    is_mut: a.writable,
                    is_signer: a.signer,
                    is_optional: false,
                    pda: a
                        .pda
                        .as_ref()
                        .and_then(|p| serde_json::to_value(p).ok()),
                })
                .collect(),
        })
        .collect()
}

/// Build the list of PDA accounts used by `cmd_pda` (file mode).
/// Each IDL account that carries a `pda` definition becomes a `PdaAccount`.
#[allow(dead_code)]
pub fn idl_to_pda_accounts(idl: &SolanaIdl) -> Vec<PdaAccount> {
    let mut result = Vec::new();
    for ix in &idl.instructions {
        for acc in &ix.accounts {
            if let Some(pda) = &acc.pda {
                let seeds: Vec<PdaSeed> = pda
                    .seeds
                    .iter()
                    .map(|s| PdaSeed {
                        kind: s.kind.clone(),
                        description: s.value.as_ref().map(|v| {
                            String::from_utf8(v.clone())
                                .unwrap_or_else(|_| format!("{v:?}"))
                        }),
                        name: s.path.clone(),
                        ty: if s.kind == "account" {
                            Some("publicKey".to_string())
                        } else if s.kind == "arg" {
                            // Look up arg type from the instruction definition.
                            ix.args
                                .iter()
                                .find(|a| Some(a.name.as_str()) == s.path.as_deref())
                                .map(|a| match &a.ty {
                                    Value::String(t) => t.clone(),
                                    _ => "string".to_string(),
                                })
                        } else {
                            None
                        },
                    })
                    .collect();

                result.push(PdaAccount {
                    instruction: ix.name.clone(),
                    account: acc.name.clone(),
                    seeds,
                });
            }
        }
    }
    result
}

// ── PDA seed resolution ───────────────────────────────────────────────────────

/// Attempt to resolve all seeds of a PDA definition into raw byte slices.
/// Returns `None` if any seed value is not yet available (not collected yet).
///
/// `collected_accounts`: map of account name → base58 pubkey collected so far.
/// `collected_args`:     map of arg name → JSON value collected so far.
pub fn resolve_pda_seeds(
    pda: &IdlPda,
    ix: &IdlInstruction,
    collected_accounts: &HashMap<String, String>,
    collected_args: &HashMap<String, serde_json::Value>,
) -> Option<Vec<Vec<u8>>> {
    let mut seeds: Vec<Vec<u8>> = Vec::new();
    for seed in &pda.seeds {
        match seed.kind.as_str() {
            "const" => {
                seeds.push(seed.value.clone()?);
            }
            "arg" => {
                let path = seed.path.as_deref()?;
                let val = collected_args.get(path)?;
                // Find the arg type from the instruction definition.
                let ty = ix
                    .args
                    .iter()
                    .find(|a| a.name == path)
                    .and_then(|a| match &a.ty {
                        Value::String(s) => Some(s.clone()),
                        _ => None,
                    })
                    .unwrap_or_else(|| "string".to_string());
                seeds.push(seed_bytes_from_value(val, &ty).ok()?);
            }
            "account" => {
                let path = seed.path.as_deref()?;
                let pubkey_str = collected_accounts.get(path)?;
                let bytes = bs58::decode(pubkey_str).into_vec().ok()?;
                if bytes.len() != 32 {
                    return None;
                }
                seeds.push(bytes);
            }
            _ => return None,
        }
    }
    Some(seeds)
}

/// Encode a single value as raw PDA seed bytes (NOT Borsh-prefixed for strings).
/// Strings → raw UTF-8, numerics → little-endian, pubkeys → 32 raw bytes.
pub fn seed_bytes_from_value(val: &serde_json::Value, ty: &str) -> Result<Vec<u8>> {
    match ty.to_lowercase().as_str() {
        "string" => {
            let s = val.as_str().unwrap_or_default();
            Ok(s.as_bytes().to_vec())
        }
        "u64" => {
            let n = val_as_u64(val).context("Expected u64")?;
            Ok(n.to_le_bytes().to_vec())
        }
        "u32" => {
            let n = val_as_u64(val).context("Expected u32")?;
            Ok((n as u32).to_le_bytes().to_vec())
        }
        "u16" => {
            let n = val_as_u64(val).context("Expected u16")?;
            Ok((n as u16).to_le_bytes().to_vec())
        }
        "u8" => {
            let n = val_as_u64(val).context("Expected u8")?;
            Ok(vec![n as u8])
        }
        "i64" => {
            let n = val_as_i64(val).context("Expected i64")?;
            Ok(n.to_le_bytes().to_vec())
        }
        "i32" => {
            let n = val_as_i64(val).context("Expected i32")?;
            Ok((n as i32).to_le_bytes().to_vec())
        }
        "pubkey" | "publickey" => {
            let s = val.as_str().context("Expected pubkey string")?;
            let bytes = bs58::decode(s)
                .into_vec()
                .with_context(|| format!("Invalid pubkey: {s}"))?;
            anyhow::ensure!(bytes.len() == 32, "Pubkey must decode to 32 bytes");
            Ok(bytes)
        }
        _ => bail!("Unknown seed type '{ty}'; cannot encode PDA seed"),
    }
}

// ── Borsh encoding ────────────────────────────────────────────────────────────

/// Borsh-encode a single value according to its IDL type string.
///
/// Type mapping:
///  - `string`  → u32 LE length prefix + UTF-8 bytes
///  - `u8/u16/u32/u64/u128` → LE bytes
///  - `i8/i16/i32/i64/i128` → LE bytes
///  - `bool`    → single byte (0 or 1)
///  - `pubkey`  → 32 raw bytes (base58 decoded)
pub fn borsh_encode_value(val: &serde_json::Value, ty: &str) -> Result<Vec<u8>> {
    match ty.to_lowercase().as_str() {
        "string" => {
            let s = match val {
                Value::String(s) => s.clone(),
                v => v.to_string(),
            };
            let b = s.as_bytes();
            let mut out = Vec::with_capacity(4 + b.len());
            out.extend_from_slice(&(b.len() as u32).to_le_bytes());
            out.extend_from_slice(b);
            Ok(out)
        }
        "u128" => {
            let n = val_as_u128(val).context("Expected u128")?;
            Ok(n.to_le_bytes().to_vec())
        }
        "u64" => {
            let n = val_as_u64(val).context("Expected u64")?;
            Ok(n.to_le_bytes().to_vec())
        }
        "u32" => {
            let n = val_as_u64(val).context("Expected u32")?;
            Ok((n as u32).to_le_bytes().to_vec())
        }
        "u16" => {
            let n = val_as_u64(val).context("Expected u16")?;
            Ok((n as u16).to_le_bytes().to_vec())
        }
        "u8" => {
            let n = val_as_u64(val).context("Expected u8")?;
            Ok(vec![n as u8])
        }
        "i128" => {
            let n = val_as_i128(val).context("Expected i128")?;
            Ok(n.to_le_bytes().to_vec())
        }
        "i64" => {
            let n = val_as_i64(val).context("Expected i64")?;
            Ok(n.to_le_bytes().to_vec())
        }
        "i32" => {
            let n = val_as_i64(val).context("Expected i32")?;
            Ok((n as i32).to_le_bytes().to_vec())
        }
        "i16" => {
            let n = val_as_i64(val).context("Expected i16")?;
            Ok((n as i16).to_le_bytes().to_vec())
        }
        "i8" => {
            let n = val_as_i64(val).context("Expected i8")?;
            Ok(vec![n as i8 as u8])
        }
        "bool" => {
            let b = match val {
                Value::Bool(b) => *b,
                Value::String(s) => matches!(s.to_lowercase().as_str(), "true" | "yes" | "1"),
                Value::Number(n) => n.as_u64().unwrap_or(0) != 0,
                _ => false,
            };
            Ok(vec![b as u8])
        }
        "pubkey" | "publickey" => {
            let s = val.as_str().context("Expected pubkey string")?;
            let bytes = bs58::decode(s)
                .into_vec()
                .with_context(|| format!("Invalid pubkey: {s}"))?;
            anyhow::ensure!(bytes.len() == 32, "Pubkey {s} must decode to 32 bytes");
            Ok(bytes)
        }
        _ => bail!(
            "Unsupported Borsh type '{ty}'. \
             Complex/nested types require the Orquestra API mode. \
             Remove --idl or set idl_path to empty to use the API."
        ),
    }
}

/// Borsh-encode all instruction args in IDL order.
pub fn borsh_encode_args(
    arg_defs: &[IdlArg],
    values: &HashMap<String, serde_json::Value>,
) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    for arg in arg_defs {
        let ty_str = match &arg.ty {
            Value::String(s) => s.clone(),
            v => bail!(
                "Complex arg type not yet supported in file mode: {v}. \
                 Use the Orquestra API mode for this instruction."
            ),
        };
        let val = values
            .get(&arg.name)
            .with_context(|| format!("Missing value for arg '{}'", arg.name))?;
        let encoded = borsh_encode_value(val, &ty_str)
            .with_context(|| format!("Encoding arg '{}' ({})", arg.name, ty_str))?;
        out.extend(encoded);
    }
    Ok(out)
}

/// Prepend the 8-byte discriminator to Borsh-encoded arg bytes.
pub fn build_instruction_data(discriminator: &[u8], arg_bytes: &[u8]) -> Vec<u8> {
    let mut data = Vec::with_capacity(discriminator.len() + arg_bytes.len());
    data.extend_from_slice(discriminator);
    data.extend_from_slice(arg_bytes);
    data
}

// ── Numeric value helpers ─────────────────────────────────────────────────────

fn val_as_u64(val: &Value) -> Option<u64> {
    match val {
        Value::Number(n) => n.as_u64(),
        Value::String(s) => s.parse().ok(),
        _ => None,
    }
}

fn val_as_i64(val: &Value) -> Option<i64> {
    match val {
        Value::Number(n) => n.as_i64(),
        Value::String(s) => s.parse().ok(),
        _ => None,
    }
}

fn val_as_u128(val: &Value) -> Option<u128> {
    match val {
        Value::String(s) => s.parse().ok(),
        Value::Number(n) => n.as_u64().map(|n| n as u128),
        _ => None,
    }
}

fn val_as_i128(val: &Value) -> Option<i128> {
    match val {
        Value::String(s) => s.parse().ok(),
        Value::Number(n) => n.as_i64().map(|n| n as i128),
        _ => None,
    }
}
