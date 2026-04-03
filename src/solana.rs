use anyhow::{bail, Context, Result};
use serde::Deserialize;
use serde_json::{json, Value};

/// Load keypair from a standard Solana CLI JSON file ([u8; 64] byte array)
pub fn load_keypair(path: &str) -> Result<[u8; 64]> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Cannot read keypair file: {path}"))?;
    let bytes: Vec<u8> = serde_json::from_str(&content)
        .with_context(|| format!("Keypair file is not a valid JSON byte array: {path}"))?;
    if bytes.len() != 64 {
        bail!("Keypair file must contain exactly 64 bytes, got {}", bytes.len());
    }
    let mut arr = [0u8; 64];
    arr.copy_from_slice(&bytes);
    Ok(arr)
}

/// Derive the base58 public key from a keypair file (last 32 bytes = public key)
pub fn pubkey_from_keypair_file(path: &str) -> Result<String> {
    let bytes = load_keypair(path)?;
    Ok(bs58::encode(&bytes[32..]).into_string())
}

/// Sign a base58-encoded unsigned transaction and send it to the Solana RPC.
/// Returns the transaction signature.
pub async fn sign_and_send(
    base58_tx: &str,
    keypair_path: &str,
    rpc_url: &str,
    _fee_payer: &str,
) -> Result<String> {
    let keypair_bytes = load_keypair(keypair_path)?;

    // 1. Decode base58 transaction
    let tx_bytes = bs58::decode(base58_tx)
        .into_vec()
        .context("Invalid base58 transaction")?;

    // 2. Get latest blockhash from RPC
    let (blockhash_bytes, _) = get_latest_blockhash(rpc_url).await?;

    // 3. Parse wire format: [sig_count compact-u16] [sigs...] [message...]
    let (sig_count, sig_count_len) = read_compact_u16(&tx_bytes, 0)?;
    let sigs_end = sig_count_len + (sig_count as usize) * 64;
    let mut message = tx_bytes[sigs_end..].to_vec();

    // 4. Find blockhash offset in message header:
    //    header: 3 bytes, compact-u16 account_count, account_count*32 bytes, then 32-byte blockhash
    let header_len = 3usize;
    let (account_count, ac_len) = read_compact_u16(&message, header_len)?;
    let accounts_end = header_len + ac_len + (account_count as usize) * 32;

    if accounts_end + 32 > message.len() {
        bail!("Transaction message too short to contain blockhash");
    }

    // 5. Replace blockhash with fresh one
    message[accounts_end..accounts_end + 32].copy_from_slice(&blockhash_bytes);

    // 6. Sign message with ed25519
    let sig = sign_message(&keypair_bytes, &message)?;

    // 7. Reassemble: [compact-u16 = 1] [sig 64 bytes] [message]
    let mut signed: Vec<u8> = Vec::with_capacity(1 + 64 + message.len());
    signed.push(1u8); // compact-u16 for value 1
    signed.extend_from_slice(&sig);
    signed.extend_from_slice(&message);

    // 8. Send via JSON-RPC
    let encoded = bs58::encode(&signed).into_string();
    send_raw_transaction(rpc_url, &encoded).await
}

/// Ed25519 signing using ed25519-dalek (SigningKey takes the 32-byte seed)
fn sign_message(keypair_bytes: &[u8; 64], message: &[u8]) -> Result<[u8; 64]> {
    use ed25519_dalek::{Signer, SigningKey};

    let seed: [u8; 32] = keypair_bytes[..32]
        .try_into()
        .context("Invalid secret key length")?;
    let signing_key = SigningKey::from_bytes(&seed);
    Ok(signing_key.sign(message).to_bytes())
}

// ── RPC helpers ──────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct RpcBlockhashResponse {
    result: Option<BlockhashResult>,
    error: Option<Value>,
}

#[derive(Deserialize)]
struct BlockhashResult {
    value: BlockhashValue,
}

#[derive(Deserialize)]
struct BlockhashValue {
    blockhash: String,
    #[serde(rename = "lastValidBlockHeight")]
    last_valid_block_height: u64,
}

async fn get_latest_blockhash(rpc_url: &str) -> Result<([u8; 32], u64)> {
    let client = reqwest::Client::new();
    let body = json!({
        "jsonrpc": "2.0", "id": 1,
        "method": "getLatestBlockhash",
        "params": [{"commitment": "confirmed"}]
    });

    let resp: RpcBlockhashResponse = client
        .post(rpc_url)
        .json(&body)
        .send()
        .await
        .context("RPC getLatestBlockhash request failed")?
        .json()
        .await
        .context("Failed to parse getLatestBlockhash response")?;

    if let Some(err) = resp.error {
        bail!("RPC error: {err}");
    }

    let result = resp.result.context("No result in getLatestBlockhash response")?;
    let bh_bytes = bs58::decode(&result.value.blockhash)
        .into_vec()
        .context("Invalid blockhash base58")?;
    if bh_bytes.len() != 32 {
        bail!("Blockhash must be 32 bytes");
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bh_bytes);
    Ok((arr, result.value.last_valid_block_height))
}

#[derive(Deserialize)]
struct RpcSendResponse {
    result: Option<String>,
    error: Option<Value>,
}

async fn send_raw_transaction(rpc_url: &str, encoded_tx: &str) -> Result<String> {
    let client = reqwest::Client::new();
    let body = json!({
        "jsonrpc": "2.0", "id": 1,
        "method": "sendTransaction",
        "params": [encoded_tx, {"encoding": "base58", "preflightCommitment": "confirmed"}]
    });

    let resp: RpcSendResponse = client
        .post(rpc_url)
        .json(&body)
        .send()
        .await
        .context("RPC sendTransaction request failed")?
        .json()
        .await
        .context("Failed to parse sendTransaction response")?;

    if let Some(err) = resp.error {
        bail!("RPC sendTransaction error: {err}");
    }

    resp.result.context("No signature in sendTransaction response")
}

/// Read a Solana compact-u16. Returns (value, bytes_consumed).
fn read_compact_u16(bytes: &[u8], offset: usize) -> Result<(u16, usize)> {
    let mut val: u16 = 0;
    let mut shift = 0u16;
    let mut consumed = 0;
    loop {
        if offset + consumed >= bytes.len() {
            bail!("Unexpected end of bytes reading compact-u16");
        }
        let byte = bytes[offset + consumed];
        consumed += 1;
        val |= ((byte & 0x7f) as u16) << shift;
        if byte & 0x80 == 0 {
            break;
        }
        shift += 7;
        if shift >= 16 {
            bail!("compact-u16 overflow");
        }
    }
    Ok((val, consumed))
}
