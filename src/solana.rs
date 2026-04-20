use anyhow::{bail, Context, Result};
use serde::Deserialize;
use serde_json::{json, Value};

// ── PDA derivation ────────────────────────────────────────────────────────────

/// Solana `create_program_address` — SHA256(seeds... || program_id || "ProgramDerivedAddress")
/// and verify the resulting 32 bytes are NOT a valid ed25519 point.
fn create_program_address(seeds: &[&[u8]], program_id: &[u8; 32]) -> Option<[u8; 32]> {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    for seed in seeds {
        hasher.update(seed);
    }
    hasher.update(program_id);
    hasher.update(b"ProgramDerivedAddress");
    let result: [u8; 32] = hasher.finalize().into();

    // Valid PDA must be off the ed25519 curve.
    let compressed = curve25519_dalek::edwards::CompressedEdwardsY(result);
    if compressed.decompress().is_some() {
        return None; // on-curve → invalid PDA
    }
    Some(result)
}

/// Find a valid program-derived address by iterating bump from 255 → 0.
/// Returns `(base58_address, bump)`.
pub fn find_program_address(seeds: &[Vec<u8>], program_id_b58: &str) -> Result<(String, u8)> {
    let prog_bytes = bs58::decode(program_id_b58)
        .into_vec()
        .with_context(|| format!("Invalid program ID base58: {program_id_b58}"))?;
    let prog: [u8; 32] = prog_bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("Program ID must be 32 bytes"))?;

    for bump in (0..=255u8).rev() {
        let mut with_bump: Vec<Vec<u8>> = seeds.to_vec();
        with_bump.push(vec![bump]);
        let refs: Vec<&[u8]> = with_bump.iter().map(|s| s.as_slice()).collect();
        if let Some(addr) = create_program_address(&refs, &prog) {
            return Ok((bs58::encode(addr).into_string(), bump));
        }
    }
    bail!("Could not find a valid program-derived address (all bumps exhausted)")
}

// ── Unsigned message builder ──────────────────────────────────────────────────

/// Build a signed-transaction-ready binary Solana **legacy message** with a
/// zeroed recent-blockhash placeholder.  The caller can hand this base58 string
/// directly to `sign_and_send`, which will:
///   1. Decode the base58 binary.
///   2. Skip the JSON-parse path (it's not JSON).
///   3. Call `extract_message_bytes` + `blockhash_offset` to locate the
///      placeholder and overwrite it with a fresh blockhash.
///   4. Sign + broadcast.
///
/// `accounts` should be the instruction's account list **in IDL order** as
/// `(pubkey_base58, is_signer, is_writable)`.  The function deduplicates
/// internally and sorts into canonical Solana ordering.
pub fn encode_unsigned_message(
    fee_payer: &str,
    program_id: &str,
    accounts: &[(String, bool, bool)],
    instruction_data: &[u8],
) -> Result<String> {
    // Build the full deduplicated account list.
    // Order: fee payer, then instruction accounts, then program id.
    let mut all: Vec<(String, bool, bool)> = Vec::new();
    let mut seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

    macro_rules! add_acc {
        ($key:expr, $signer:expr, $writable:expr) => {{
            let k = $key.to_string();
            if let Some(&idx) = seen.get(&k) {
                all[idx].1 |= $signer;
                all[idx].2 |= $writable;
            } else {
                let idx = all.len();
                all.push((k.clone(), $signer, $writable));
                seen.insert(k, idx);
            }
        }};
    }

    add_acc!(fee_payer, true, true);
    for (pk, is_signer, is_writable) in accounts {
        add_acc!(pk, *is_signer, *is_writable);
    }
    add_acc!(program_id, false, false);

    // Sort into canonical Solana order, keeping fee payer first.
    let fp = all.remove(0);
    all.sort_by_key(|(_, s, w)| match (*s, *w) {
        (true, true) => 0u8,
        (true, false) => 1,
        (false, true) => 2,
        (false, false) => 3,
    });
    all.insert(0, fp);

    // Build index map.
    let index_map: std::collections::HashMap<&str, u8> = all
        .iter()
        .enumerate()
        .map(|(i, (k, _, _))| (k.as_str(), i as u8))
        .collect();

    let num_signers = all.iter().filter(|(_, s, _)| *s).count() as u8;
    let num_ro_signed = all.iter().filter(|(_, s, w)| *s && !*w).count() as u8;
    let num_ro_unsigned = all.iter().filter(|(_, s, w)| !*s && !*w).count() as u8;

    let mut msg: Vec<u8> = Vec::new();

    // 3-byte message header.
    msg.push(num_signers);
    msg.push(num_ro_signed);
    msg.push(num_ro_unsigned);

    // Account keys.
    write_compact_u16(&mut msg, all.len() as u16);
    for (pk, _, _) in &all {
        let bytes = bs58::decode(pk)
            .into_vec()
            .with_context(|| format!("Invalid pubkey: {pk}"))?;
        anyhow::ensure!(bytes.len() == 32, "Pubkey {pk} must decode to 32 bytes");
        msg.extend_from_slice(&bytes);
    }

    // Recent blockhash placeholder — 32 zero bytes; `sign_and_send` patches this.
    msg.extend_from_slice(&[0u8; 32]);

    // Single instruction.
    write_compact_u16(&mut msg, 1u16);

    let prog_idx = *index_map
        .get(program_id)
        .with_context(|| format!("Program ID {program_id} not found in account list"))?;
    msg.push(prog_idx);

    // Account indices (in IDL order, matching `accounts` slice).
    write_compact_u16(&mut msg, accounts.len() as u16);
    for (pk, _, _) in accounts {
        let idx = *index_map
            .get(pk.as_str())
            .with_context(|| format!("Account {pk} not found in account list"))?;
        msg.push(idx);
    }

    // Instruction data.
    write_compact_u16(&mut msg, instruction_data.len() as u16);
    msg.extend_from_slice(instruction_data);

    Ok(bs58::encode(&msg).into_string())
}


pub fn load_keypair(path: &str) -> Result<[u8; 64]> {
    let normalized_path = path.trim();
    let content = std::fs::read_to_string(normalized_path)
        .with_context(|| format!("Cannot read keypair file: {normalized_path}"))?;
    let bytes: Vec<u8> = serde_json::from_str(&content)
        .with_context(|| format!("Keypair file is not a valid JSON byte array: {normalized_path}"))?;
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

// ── Transaction JSON types (Orquestra build API format) ─────────────────────

#[derive(Deserialize)]
struct TxJson {
    #[serde(rename = "feePayer")]
    fee_payer: String,
    #[allow(dead_code)]
    #[serde(rename = "recentBlockhash")]
    recent_blockhash: String,
    instructions: Vec<IxJson>,
}

#[derive(Deserialize)]
struct IxJson {
    #[serde(rename = "programId")]
    program_id: String,
    #[serde(default)]
    keys: Vec<KeyJson>,
    #[serde(default)]
    data: String,
}

#[derive(Deserialize)]
struct KeyJson {
    pubkey: String,
    #[serde(rename = "isSigner", default)]
    is_signer: bool,
    #[serde(rename = "isWritable", default)]
    is_writable: bool,
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

    // 1. Decode the encoded string (base58 or base64) into raw bytes.
    let tx_bytes = decode_transaction_bytes(base58_tx)?;

    // 2. Get a fresh blockhash from the RPC.
    let (blockhash_bytes, _) = get_latest_blockhash(rpc_url).await?;

    // 3. Build the binary Solana message.
    //    Orquestra's build API returns a JSON object encoded as base58.
    //    If the decoded bytes are valid JSON we reconstruct the canonical
    //    binary message from scratch and inject the fresh blockhash directly.
    //    Otherwise fall back to treating the bytes as a binary wire transaction.
    let message: Vec<u8> = if let Ok(tx_json) = serde_json::from_slice::<TxJson>(&tx_bytes) {
        build_message_from_json(&tx_json, &blockhash_bytes)?
    } else {
        let mut msg = extract_message_bytes(&tx_bytes)?;
        let accounts_end = blockhash_offset(&msg)?;
        msg[accounts_end..accounts_end + 32].copy_from_slice(&blockhash_bytes);
        msg
    };

    // 4. Sign with ed25519.
    let sig = sign_message(&keypair_bytes, &message)?;

    // 5. Reassemble wire transaction: [compact-u16 = 1] [sig 64 bytes] [message]
    let mut signed: Vec<u8> = Vec::with_capacity(1 + 64 + message.len());
    signed.push(1u8); // compact-u16 encoding of 1
    signed.extend_from_slice(&sig);
    signed.extend_from_slice(&message);

    // 6. Base58-encode and send.
    let encoded = bs58::encode(&signed).into_string();
    send_raw_transaction(rpc_url, &encoded).await
}

/// Build a canonical Solana legacy message binary from the Orquestra JSON format.
/// Accounts are sorted per Solana spec:
///   1. writable signers (fee payer always first)
///   2. read-only signers
///   3. writable non-signers
///   4. read-only non-signers (program IDs land here)
fn build_message_from_json(tx: &TxJson, fresh_blockhash: &[u8; 32]) -> Result<Vec<u8>> {
    use std::collections::HashMap;

    // Ordered list: (pubkey, is_signer, is_writable)
    let mut accounts: Vec<(String, bool, bool)> = Vec::new();
    let mut seen: HashMap<String, usize> = HashMap::new();

    macro_rules! add_account {
        ($pubkey:expr, $signer:expr, $writable:expr) => {{
            let key = $pubkey.to_string();
            if let Some(&idx) = seen.get(&key) {
                accounts[idx].1 |= $signer;
                accounts[idx].2 |= $writable;
            } else {
                let idx = accounts.len();
                accounts.push((key.clone(), $signer, $writable));
                seen.insert(key, idx);
            }
        }};
    }

    // Fee payer must always be first and is a writable signer.
    add_account!(&tx.fee_payer, true, true);

    for ix in &tx.instructions {
        for key in &ix.keys {
            add_account!(&key.pubkey, key.is_signer, key.is_writable);
        }
        // Program IDs are read-only non-signers.
        add_account!(&ix.program_id, false, false);
    }

    // Sort into canonical order, keeping fee payer at index 0.
    let fee_payer_entry = accounts.remove(0);
    accounts.sort_by_key(|(_, is_signer, is_writable)| match (*is_signer, *is_writable) {
        (true, true)   => 0u8,
        (true, false)  => 1,
        (false, true)  => 2,
        (false, false) => 3,
    });
    accounts.insert(0, fee_payer_entry);

    // Build an index map for fast O(1) lookup.
    let index_map: HashMap<&str, u8> = accounts
        .iter()
        .enumerate()
        .map(|(i, (k, _, _))| (k.as_str(), i as u8))
        .collect();

    let num_signers       = accounts.iter().filter(|(_, s, _)| *s).count();
    let num_ro_signed     = accounts.iter().filter(|(_, s, w)| *s && !*w).count();
    let num_ro_unsigned   = accounts.iter().filter(|(_, s, w)| !*s && !*w).count();

    let mut msg: Vec<u8> = Vec::new();

    // Header (3 bytes)
    msg.push(num_signers     as u8);
    msg.push(num_ro_signed   as u8);
    msg.push(num_ro_unsigned as u8);

    // Account keys
    write_compact_u16(&mut msg, accounts.len() as u16);
    for (pubkey_str, _, _) in &accounts {
        let bytes = bs58::decode(pubkey_str)
            .into_vec()
            .with_context(|| format!("Invalid pubkey: {pubkey_str}"))?;
        if bytes.len() != 32 {
            bail!("Pubkey {pubkey_str} decoded to {} bytes, expected 32", bytes.len());
        }
        msg.extend_from_slice(&bytes);
    }

    // Blockhash (fresh, 32 bytes)
    msg.extend_from_slice(fresh_blockhash);

    // Instructions
    write_compact_u16(&mut msg, tx.instructions.len() as u16);
    for ix in &tx.instructions {
        let prog_idx = *index_map
            .get(ix.program_id.as_str())
            .with_context(|| format!("Program ID {} not in account list", ix.program_id))?;
        msg.push(prog_idx);

        write_compact_u16(&mut msg, ix.keys.len() as u16);
        for key in &ix.keys {
            let idx = *index_map
                .get(key.pubkey.as_str())
                .with_context(|| format!("Account {} not in account list", key.pubkey))?;
            msg.push(idx);
        }

        let data = decode_instruction_data(&ix.data)?;
        write_compact_u16(&mut msg, data.len() as u16);
        msg.extend_from_slice(&data);
    }

    Ok(msg)
}

/// Decode instruction data that may be base58 or base64 encoded.
fn decode_instruction_data(encoded: &str) -> Result<Vec<u8>> {
    use base64::Engine;
    if encoded.is_empty() {
        return Ok(vec![]);
    }
    if let Ok(b) = bs58::decode(encoded).into_vec() {
        return Ok(b);
    }
    if let Ok(b) = base64::engine::general_purpose::STANDARD.decode(encoded) {
        return Ok(b);
    }
    bail!("Cannot decode instruction data (not base58 or base64): {encoded}")
}

/// Write a Solana compact-u16 into a byte buffer.
fn write_compact_u16(buf: &mut Vec<u8>, val: u16) {
    let mut v = val;
    loop {
        let mut byte = (v & 0x7f) as u8;
        v >>= 7;
        if v > 0 {
            byte |= 0x80;
        }
        buf.push(byte);
        if v == 0 {
            break;
        }
    }
}

/// Decode an encoded transaction string.
/// Tries base58 first (legacy Solana), then base64 standard, then base64 URL-safe.
/// Returns the raw bytes and the encoding name used (for diagnostics).
fn decode_transaction_bytes(encoded: &str) -> Result<Vec<u8>> {
    use base64::Engine;

    // Attempt 1: base58
    if let Ok(bytes) = bs58::decode(encoded).into_vec() {
        if !bytes.is_empty() {
            return Ok(bytes);
        }
    }

    // Attempt 2: base64 standard
    if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(encoded) {
        if !bytes.is_empty() {
            return Ok(bytes);
        }
    }

    // Attempt 3: base64 URL-safe (no padding)
    if let Ok(bytes) = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(encoded) {
        if !bytes.is_empty() {
            return Ok(bytes);
        }
    }

    bail!(
        "Transaction string is not valid base58 or base64 (len={})",
        encoded.len()
    );
}

fn extract_message_bytes(tx_bytes: &[u8]) -> Result<Vec<u8>> {
    if tx_bytes.is_empty() {
        bail!("Transaction payload is empty");
    }

    // Attempt 1: treat as a full wire transaction whose first compact-u16 encodes
    // the number of existing signatures.  Strip that section and validate the
    // remaining bytes as a message.  This is tried first because a wire tx with
    // 1 signature (the common case) starts with 0x01 which also happens to look
    // like a valid legacy-message header (num_signers=1, 0 accounts), causing the
    // raw-message path below to pass with a garbage blockhash offset.
    if let Ok((sig_count, sig_count_len)) = read_compact_u16(tx_bytes, 0) {
        if sig_count > 0 {
            if let Some(sigs_end) = sig_count_len.checked_add((sig_count as usize).saturating_mul(64)) {
                if sigs_end < tx_bytes.len() {
                    let candidate = &tx_bytes[sigs_end..];
                    if blockhash_offset(candidate).is_ok() {
                        return Ok(candidate.to_vec());
                    }
                }
            }
        }
    }

    // Attempt 2: treat whole payload as a raw unsigned message (produced by
    // encode_unsigned_message or similar APIs that return a message directly
    // without a prepended signature section).
    if blockhash_offset(tx_bytes).is_ok() {
        return Ok(tx_bytes.to_vec());
    }

    let hex_preview: String = tx_bytes
        .iter()
        .take(16)
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ");

    bail!(
        "Cannot determine transaction message format (payload_len={}, first_bytes=[{}]). \
         Payload does not parse as either a raw Solana message or a signed wire transaction.",
        tx_bytes.len(),
        hex_preview
    );
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

/// Returns the byte offset where account keys end and blockhash starts.
/// Supports both legacy and versioned message formats.
fn blockhash_offset(message: &[u8]) -> Result<usize> {
    if message.is_empty() {
        bail!("Transaction message is empty");
    }

    // Legacy message starts directly with 3-byte header.
    // Versioned message starts with 1-byte prefix (MSB set), then 3-byte header.
    let header_start = if message[0] & 0x80 != 0 {
        if message.len() < 4 {
            bail!("Versioned transaction message too short for header");
        }
        1usize
    } else {
        0usize
    };

    let account_count_offset = header_start + 3;
    if account_count_offset >= message.len() {
        bail!("Transaction message too short to contain account key count");
    }

    let (account_count, ac_len) = read_compact_u16(message, account_count_offset)?;
    let accounts_start = account_count_offset
        .checked_add(ac_len)
        .context("Transaction message account key section overflow")?;
    let key_bytes = (account_count as usize)
        .checked_mul(32)
        .context("Transaction account key bytes overflow")?;

    let accounts_end = accounts_start
        .checked_add(key_bytes)
        .context("Transaction message account key end overflow")?;

    // At minimum, blockhash (32 bytes) must follow account keys.
    if accounts_end + 32 > message.len() {
        bail!(
            "Invalid transaction payload: parsed account_count={} requires >= {} bytes before/including blockhash, but message_len={}",
            account_count,
            accounts_end + 32,
            message.len()
        );
    }

    Ok(accounts_end)
}

// ── simulate_transaction ─────────────────────────────────────────────────────

#[derive(Deserialize, Debug)]
pub struct SimulateValue {
    pub err: Option<Value>,
    pub logs: Option<Vec<String>>,
    #[serde(rename = "unitsConsumed")]
    pub units_consumed: Option<u64>,
}

#[derive(Deserialize)]
struct SimulateResponse {
    result: Option<SimulateResult>,
    error: Option<Value>,
}

#[derive(Deserialize)]
struct SimulateResult {
    value: SimulateValue,
}

/// Simulate a transaction via RPC without signing or sending.
/// Accepts the same base58/base64 encoded tx formats as `sign_and_send`.
pub async fn simulate_transaction(base58_tx: &str, rpc_url: &str) -> Result<SimulateValue> {
    let tx_bytes = decode_transaction_bytes(base58_tx)?;

    // Build a wire tx with a zeroed sig for simulate.
    // replaceRecentBlockhash:true means RPC will patch the blockhash, so zeros are fine.
    let message = if let Ok(tx_json) = serde_json::from_slice::<TxJson>(&tx_bytes) {
        build_message_from_json(&tx_json, &[0u8; 32])?
    } else {
        extract_message_bytes(&tx_bytes)?
    };

    let mut wire: Vec<u8> = Vec::with_capacity(1 + 64 + message.len());
    wire.push(1u8);
    wire.extend_from_slice(&[0u8; 64]); // zeroed dummy signature
    wire.extend_from_slice(&message);
    let encoded = bs58::encode(&wire).into_string();

    let client = reqwest::Client::new();
    let body = json!({
        "jsonrpc": "2.0", "id": 1,
        "method": "simulateTransaction",
        "params": [encoded, {
            "encoding": "base58",
            "commitment": "confirmed",
            "sigVerify": false,
            "replaceRecentBlockhash": true
        }]
    });

    let resp: SimulateResponse = client
        .post(rpc_url)
        .json(&body)
        .send()
        .await
        .context("RPC simulateTransaction failed")?
        .json()
        .await
        .context("Failed to parse simulateTransaction response")?;

    if let Some(err) = resp.error {
        bail!("RPC error: {err}");
    }

    resp.result
        .map(|r| r.value)
        .context("No result in simulateTransaction response")
}

// ── get_transaction ──────────────────────────────────────────────────────────

#[derive(Deserialize, Debug)]
pub struct TxMeta {
    pub err: Option<Value>,
    pub fee: Option<u64>,
    #[serde(rename = "logMessages")]
    pub log_messages: Option<Vec<String>>,
    #[serde(rename = "computeUnitsConsumed")]
    pub compute_units_consumed: Option<u64>,
}

#[derive(Deserialize, Debug)]
pub struct TxMessageInfo {
    /// Flat account key list (JSON encoding returns strings)
    #[serde(rename = "accountKeys")]
    pub account_keys: Vec<Value>,
    pub instructions: Vec<Value>,
}

#[derive(Deserialize, Debug)]
pub struct GetTransactionData {
    pub message: TxMessageInfo,
}

#[derive(Deserialize, Debug)]
pub struct GetTransactionResult {
    pub transaction: GetTransactionData,
    pub meta: Option<TxMeta>,
    #[serde(rename = "blockTime")]
    pub block_time: Option<i64>,
    pub slot: Option<u64>,
}

#[derive(Deserialize)]
struct GetTransactionResponse {
    result: Option<GetTransactionResult>,
    error: Option<Value>,
}

/// Fetch a confirmed transaction by its base58 signature.
pub async fn get_transaction(signature: &str, rpc_url: &str) -> Result<GetTransactionResult> {
    let client = reqwest::Client::new();
    let body = json!({
        "jsonrpc": "2.0", "id": 1,
        "method": "getTransaction",
        "params": [signature, {
            "encoding": "json",
            "commitment": "confirmed",
            "maxSupportedTransactionVersion": 0
        }]
    });

    let resp: GetTransactionResponse = client
        .post(rpc_url)
        .json(&body)
        .send()
        .await
        .context("RPC getTransaction failed")?
        .json()
        .await
        .context("Failed to parse getTransaction response")?;

    if let Some(err) = resp.error {
        bail!("RPC error: {err}");
    }

    resp.result
        .context("Transaction not found (check the signature and commitment level)")
}
