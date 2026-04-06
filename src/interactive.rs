use anyhow::{bail, Context, Result};
use colored::Colorize;
use dialoguer::{theme::ColorfulTheme, Confirm, FuzzySelect, Input, Select};
use std::collections::HashMap;

use crate::api::{ApiClient, Instruction, InstructionAccount, InstructionArg, PdaAccount, PdaSeed};
use crate::config::Config;
use crate::idl;
use crate::solana;

// ── Top-level interactive menu ────────────────────────────────────────────────

pub async fn cmd_menu(config: &Config) -> Result<()> {
    println!(
        "\n{} {}\n",
        "orquestra".cyan().bold(),
        "— what do you want to do?".dimmed()
    );

    let options = [
        "List     — show all instructions",
        "Run      — run an instruction",
        "Find PDA  — derive a program-derived address",
        "Sign tx  — sign and send a serialized transaction",
        "Config   — view or update configuration",
        "Quit",
    ];

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Action")
        .items(&options)
        .default(0)
        .interact()?;

    match selection {
        0 => cmd_list(config).await?,
        1 => cmd_run(config, None).await?,
        2 => cmd_pda(config, None).await?,
        3 => cmd_sign_tx(config).await?,
        4 => cmd_config_menu().await?,
        _ => println!("{}", "Goodbye.".dimmed()),
    }

    Ok(())
}

pub async fn cmd_list(config: &Config) -> Result<()> {
    // File mode: parse IDL directly, no API call.
    if let Some(idl_path) = &config.idl_path {
        return cmd_list_file(idl_path).await;
    }

    let program_address = config.require_project_id()?;
    let client = ApiClient::new(config.api_base(), config.optional_api_key());

    let spinner = indicatif::ProgressBar::new_spinner();
    spinner.set_message("Resolving program...");
    spinner.enable_steady_tick(std::time::Duration::from_millis(80));
    let project = client.resolve_project_id(program_address).await;
    spinner.finish_and_clear();
    let project = project?;

    let spinner = indicatif::ProgressBar::new_spinner();
    spinner.set_message("Fetching instructions...");
    spinner.enable_steady_tick(std::time::Duration::from_millis(80));

    let instructions = client.list_instructions(&project.id).await?;
    spinner.finish_and_clear();

    if instructions.is_empty() {
        println!("{}", "No instructions found for this program.".yellow());
        return Ok(());
    }

    println!(
        "\n{} {} instructions in {} ({})\n",
        "▸".cyan().bold(),
        instructions.len().to_string().bold(),
        project.name.cyan(),
        program_address.dimmed()
    );

    let name_width = instructions.iter().map(|i| i.name.len()).max().unwrap_or(10) + 2;

    for ix in &instructions {
        let doc = ix.docs.first().map(|s| s.as_str()).unwrap_or("");
        println!(
            "  {:<width$} {}",
            ix.name.green().bold(),
            doc.dimmed(),
            width = name_width
        );
    }
    println!();
    Ok(())
}

pub async fn cmd_run(config: &Config, instruction_name: Option<&str>) -> Result<()> {
    // File mode: build transaction locally from IDL, no API call.
    if let Some(idl_path) = &config.idl_path {
        return cmd_run_file(idl_path, instruction_name, config).await;
    }

    let program_address = config.require_project_id()?;
    let client = ApiClient::new(config.api_base(), config.optional_api_key());

    let spinner = indicatif::ProgressBar::new_spinner();
    spinner.set_message("Resolving program...");
    spinner.enable_steady_tick(std::time::Duration::from_millis(80));
    let project = client.resolve_project_id(program_address).await;
    spinner.finish_and_clear();
    let project = project?;
    let project_id = &project.id;

    // Resolve instruction (from arg or interactive select)
    let ix = if let Some(name) = instruction_name {
        let spinner = indicatif::ProgressBar::new_spinner();
        spinner.set_message(format!("Fetching instruction '{name}'..."));
        spinner.enable_steady_tick(std::time::Duration::from_millis(80));
        let result = client.get_instruction(project_id, name).await;
        spinner.finish_and_clear();
        result?
    } else {
        let spinner = indicatif::ProgressBar::new_spinner();
        spinner.set_message("Fetching instructions...");
        spinner.enable_steady_tick(std::time::Duration::from_millis(80));
        let instructions = client.list_instructions(project_id).await?;
        spinner.finish_and_clear();

        if instructions.is_empty() {
            bail!("No instructions found for project '{project_id}'.");
        }

        let items: Vec<String> = instructions
            .iter()
            .map(|i| {
                let doc = i.docs.first().map(|s| format!(" — {s}")).unwrap_or_default();
                format!("{}{}", i.name, doc)
            })
            .collect();

        let selection = FuzzySelect::with_theme(&ColorfulTheme::default())
            .with_prompt("Select instruction")
            .items(&items)
            .default(0)
            .interact()?;

        instructions.into_iter().nth(selection).unwrap()
    };

    println!(
        "\n{} {}\n",
        "Instruction:".bold(),
        ix.name.green().bold()
    );
    if let Some(doc) = ix.docs.first() {
        println!("  {}\n", doc.dimmed());
    }

    // Collect args
    let args = collect_args(&ix)?;

    // Collect accounts
    let (accounts, fee_payer) = collect_accounts(&ix, config)?;

    // Confirm
    println!();
    println!("{}", "─".repeat(40).dimmed());
    println!("{}", "Summary".bold());
    println!("  Instruction : {}", ix.name.cyan());
    if !args.is_empty() {
        println!("  Args        :");
        for (k, v) in &args {
            println!("    {} = {}", k.dimmed(), v);
        }
    }
    println!("  Accounts    :");
    for (k, v) in &accounts {
        println!("    {} = {}", k.dimmed(), v);
    }
    println!("  Fee payer   : {}", fee_payer);
    println!("{}", "─".repeat(40).dimmed());
    println!();

    if !Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(format!("Build transaction for '{}'?", ix.name))
        .default(true)
        .interact()?
    {
        println!("{}", "Aborted.".yellow());
        return Ok(());
    }

    // Determine network from RPC URL
    let network = infer_network(config.rpc());

    // Build transaction
    let spinner = indicatif::ProgressBar::new_spinner();
    spinner.set_message("Building transaction...");
    spinner.enable_steady_tick(std::time::Duration::from_millis(80));

    let build = client
        .build_transaction(
            project_id,
            &ix.name,
            accounts.clone(),
            args,
            fee_payer.clone(),
            &network,
        )
        .await;
    spinner.finish_and_clear();
    let build = build?;

    println!(
        "\n{} Transaction built successfully!\n",
        "✓".green().bold()
    );

    if let Some(fee) = build.estimated_fee {
        println!("  Estimated fee : {} lamports", fee.to_string().yellow());
    }
    if let Some(serialized) = &build.serialized_transaction {
        println!("  Serialized tx : {}", serialized.dimmed());
    }

    // Sign & send if keypair is configured
    if let Some(keypair_path) = &config.keypair_path {
        println!("  Keypair found : {}", keypair_path.dimmed());
        println!();

        if Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("Sign and send transaction to Solana?")
            .default(true)
            .interact()?
        {
            let spinner = indicatif::ProgressBar::new_spinner();
            spinner.set_message("Signing and sending...");
            spinner.enable_steady_tick(std::time::Duration::from_millis(80));

            let tx_to_send = build.serialized_transaction.as_deref().unwrap_or(&build.transaction);
            let result = solana::sign_and_send(
                tx_to_send,
                keypair_path,
                config.rpc(),
                &fee_payer,
            )
            .await;
            spinner.finish_and_clear();

            match result {
                Ok(signature) => {
                    println!("{} Transaction confirmed!", "✓".green().bold());
                    println!("  Signature : {}", signature.cyan());
                    let explorer = explorer_url(&signature, &network);
                    println!("  Explorer  : {}", explorer.dimmed());
                }
                Err(e) => {
                    println!("{} Failed to send: {e}", "✗".red().bold());
                    println!("\n  Base58 tx (for manual signing):");
                    println!("  {}", build.serialized_transaction.as_deref().unwrap_or(&build.transaction).dimmed());
                }
            }
        } else {
            print_base58_tx(build.serialized_transaction.as_deref().unwrap_or(&build.transaction));
        }
    } else {
        print_base58_tx(build.serialized_transaction.as_deref().unwrap_or(&build.transaction));
    }

    Ok(())
}

fn collect_args(ix: &Instruction) -> Result<HashMap<String, serde_json::Value>> {
    if ix.args.is_empty() {
        return Ok(HashMap::new());
    }

    println!("{}", "Arguments".bold().underline());
    let mut map = HashMap::new();
    for arg in &ix.args {
        let value = prompt_arg(arg)?;
        map.insert(arg.name.clone(), value);
    }
    println!();
    Ok(map)
}

fn prompt_arg(arg: &InstructionArg) -> Result<serde_json::Value> {
    let ty_str = arg.ty.to_string();
    let prompt = format!("{} ({})", arg.name.green(), ty_str.dimmed());

    let raw: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt(&prompt)
        .interact_text()?;

    let val = coerce_value(&raw, &ty_str);
    Ok(val)
}

/// Attempt to parse input as the correct JSON type based on IDL type hint
fn coerce_value(raw: &str, ty: &str) -> serde_json::Value {
    let ty_lower = ty.to_lowercase();
    if ty_lower.contains("u8")
        || ty_lower.contains("u16")
        || ty_lower.contains("u32")
        || ty_lower.contains("u64")
        || ty_lower.contains("u128")
        || ty_lower.contains("i8")
        || ty_lower.contains("i16")
        || ty_lower.contains("i32")
        || ty_lower.contains("i64")
        || ty_lower.contains("i128")
    {
        if let Ok(n) = raw.parse::<u64>() {
            return serde_json::Value::Number(n.into());
        }
        if let Ok(n) = raw.parse::<i64>() {
            return serde_json::Value::Number(n.into());
        }
    }
    if ty_lower.contains("bool") {
        let b = matches!(raw.to_lowercase().as_str(), "true" | "yes" | "1");
        return serde_json::Value::Bool(b);
    }
    // Default: string
    serde_json::Value::String(raw.to_string())
}

fn collect_accounts(
    ix: &Instruction,
    config: &Config,
) -> Result<(HashMap<String, String>, String)> {
    // Determine fee_payer from keypair if available
    let keypair_pubkey = config
        .keypair_path
        .as_deref()
        .and_then(|p| crate::solana::pubkey_from_keypair_file(p).ok());

    let mut accounts: HashMap<String, String> = HashMap::new();
    let mut fee_payer = String::new();

    if !ix.accounts.is_empty() {
        println!("{}", "Accounts".bold().underline());
        for acc in &ix.accounts {
            let val = prompt_account(acc, keypair_pubkey.as_deref())?;
            if fee_payer.is_empty() && (acc.is_signer || acc.name.to_lowercase().contains("authority") || acc.name.to_lowercase().contains("payer")) {
                fee_payer = val.clone();
            }
            accounts.insert(acc.name.clone(), val);
        }
        println!();
    }

    // If fee_payer still empty, try keypair or prompt
    if fee_payer.is_empty() {
        fee_payer = if let Some(pk) = &keypair_pubkey {
            pk.clone()
        } else {
            Input::with_theme(&ColorfulTheme::default())
                .with_prompt("Fee payer public key")
                .interact_text()?
        };
    }

    Ok((accounts, fee_payer))
}

fn prompt_account(acc: &InstructionAccount, keypair_pubkey: Option<&str>) -> Result<String> {
    let mut flags = Vec::new();
    if acc.is_mut { flags.push("mut"); }
    if acc.is_signer { flags.push("signer"); }
    if acc.is_optional { flags.push("optional"); }
    let flags_str = if flags.is_empty() {
        String::new()
    } else {
        format!(" [{}]", flags.join(", "))
    };

    let prompt = format!("{}{}", acc.name.cyan(), flags_str.dimmed());

    // Auto-suggest keypair pubkey for signer accounts
    let default_val = if acc.is_signer {
        keypair_pubkey.map(|s| s.to_string())
    } else {
        None
    };

    let theme = ColorfulTheme::default();
    let input: Input<String> = Input::with_theme(&theme)
        .with_prompt(&prompt);

    let value: String = if let Some(def) = default_val {
        input.default(def).interact_text()?
    } else {
        input.interact_text()?
    };

    Ok(value)
}

fn infer_network(rpc: &str) -> String {
    if rpc.contains("devnet") {
        "devnet".to_string()
    } else if rpc.contains("testnet") {
        "testnet".to_string()
    } else {
        "mainnet-beta".to_string()
    }
}

fn explorer_url(signature: &str, network: &str) -> String {
    if network == "mainnet-beta" {
        format!("https://explorer.solana.com/tx/{signature}")
    } else {
        format!("https://explorer.solana.com/tx/{signature}?cluster={network}")
    }
}

fn print_base58_tx(tx: &str) {
    println!("\n{}", "Base58 encoded transaction (unsigned):".bold());
    println!("  {}", tx.dimmed());
    println!("\n  Sign with your wallet and broadcast to Solana.");
    println!(
        "  {}",
        "https://orquestra.dev/docs/sign-and-send".dimmed()
    );
}

pub async fn cmd_config_menu() -> Result<()> {
    let options = [
        "show   — display current config",
        "set    — interactively update config values",
        "back",
    ];

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Config")
        .items(&options)
        .default(0)
        .interact()?;

    match selection {
        0 => {
            let config = Config::load()?;
            println!("{}", config.display());
        }
        1 => cmd_config_reset().await?,
        _ => {}
    }

    Ok(())
}

pub async fn cmd_config_reset() -> Result<()> {
    let mut config = Config::load()?;

    println!("\n{}", "Interactive Config Setup".bold());
    println!("{}\n", "Press Enter to keep the current value, or type a new one.".dimmed());

    // project_id
    let project_id: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt(format!("{}", "Program ID (Solana pubkey)".cyan()))
        .default(config.project_id.clone().unwrap_or_default())
        .allow_empty(true)
        .interact_text()?;

    // api_key — show masked, but accept new input
    let api_key_prompt = match &config.api_key {
        Some(k) => {
            let len = k.len();
            let masked = if len <= 8 { "*".repeat(len) } else { format!("{}***{}", &k[..4], &k[len - 4..]) };
            format!("{} (current: {})", "API Key".cyan(), masked.dimmed())
        }
        None => format!("{}", "API Key".cyan()),
    };
    let api_key_raw: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt(api_key_prompt)
        .default(String::new())
        .allow_empty(true)
        .interact_text()?;

    // rpc_url
    let rpc_url: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt(format!("{}", "RPC URL".cyan()))
        .default(config.rpc_url.clone().unwrap_or_else(|| "https://api.mainnet-beta.solana.com".to_string()))
        .allow_empty(true)
        .interact_text()?;

    // keypair_path
    let keypair_path: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt(format!("{}", "Keypair path".cyan()))
        .default(config.keypair_path.clone().unwrap_or_default())
        .allow_empty(true)
        .interact_text()?;

    // api_base_url
    let api_base: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt(format!("{}", "API base URL".cyan()))
        .default(config.api_base_url.clone().unwrap_or_else(|| "https://api.orquestra.build".to_string()))
        .allow_empty(true)
        .interact_text()?;

    // idl_path
    let idl_path_input: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt(format!("{}", "IDL file path (leave empty to use API mode)".cyan()))
        .default(config.idl_path.clone().unwrap_or_default())
        .allow_empty(true)
        .interact_text()?;

    // Apply — empty string means "clear the field"
    config.project_id   = non_empty(project_id);
    // For api_key: if user typed nothing, keep existing value
    if !api_key_raw.is_empty() {
        config.api_key = Some(api_key_raw);
    }
    config.rpc_url      = non_empty(rpc_url);
    config.keypair_path = non_empty(keypair_path);
    config.api_base_url = non_empty(api_base);
    config.idl_path     = non_empty(idl_path_input);

    config.save()?;

    println!("\n{} Config saved.\n", "✓".green().bold());
    println!("{}", config.display());
    Ok(())
}

pub async fn cmd_sign_tx(config: &Config) -> Result<()> {
    let serialized_tx: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Serialized tx (base58)")
        .interact_text()?;

    let keypair_path = match &config.keypair_path {
        Some(p) => p.clone(),
        None => {
            bail!("No keypair configured. Run 'orquestra config' to set a keypair path.");
        }
    };

    let fee_payer = solana::pubkey_from_keypair_file(&keypair_path)
        .context("Failed to read public key from keypair file")?;

    let network = infer_network(config.rpc());

    let spinner = indicatif::ProgressBar::new_spinner();
    spinner.set_message("Signing and sending...");
    spinner.enable_steady_tick(std::time::Duration::from_millis(80));

    let result = solana::sign_and_send(
        &serialized_tx,
        &keypair_path,
        config.rpc(),
        &fee_payer,
    )
    .await;
    spinner.finish_and_clear();

    match result {
        Ok(signature) => {
            println!("{} Transaction confirmed!", "✓".green().bold());
            println!("  Signature : {}", signature.cyan());
            let explorer = explorer_url(&signature, &network);
            println!("  Explorer  : {}", explorer.dimmed());
        }
        Err(e) => {
            println!("{} Failed to send: {e}", "✗".red().bold());
        }
    }

    Ok(())
}

fn non_empty(s: String) -> Option<String> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

// ── PDA finder ────────────────────────────────────────────────────────────────

pub async fn cmd_pda(config: &Config, account_name: Option<&str>) -> Result<()> {
    // File mode: derive PDA locally, no API call.
    if let Some(idl_path) = &config.idl_path {
        return cmd_pda_file(idl_path, account_name).await;
    }

    let program_address = config.require_project_id()?;
    let client = ApiClient::new(config.api_base(), config.optional_api_key());

    // Resolve program address → project id
    let spinner = indicatif::ProgressBar::new_spinner();
    spinner.set_message("Resolving program...");
    spinner.enable_steady_tick(std::time::Duration::from_millis(80));
    let project = client.resolve_project_id(program_address).await;
    spinner.finish_and_clear();
    let project = project?;

    // Fetch PDA list
    let spinner = indicatif::ProgressBar::new_spinner();
    spinner.set_message("Fetching PDA accounts...");
    spinner.enable_steady_tick(std::time::Duration::from_millis(80));
    let pda_accounts = client.list_pdas(&project.id).await;
    spinner.finish_and_clear();
    let pda_accounts = pda_accounts?;

    if pda_accounts.is_empty() {
        println!("{}", "No PDA accounts found for this program.".yellow());
        return Ok(());
    }

    // Deduplicate by account name — same name across different instructions
    // means the same PDA shape; keep the first occurrence.
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut unique: Vec<&PdaAccount> = Vec::new();
    for pda in &pda_accounts {
        if seen.insert(pda.account.clone()) {
            unique.push(pda);
        }
    }

    // Select which PDA to derive
    let selected: &PdaAccount = if let Some(name) = account_name {
        unique
            .iter()
            .find(|p| p.account == name)
            .copied()
            .ok_or_else(|| anyhow::anyhow!("PDA account '{name}' not found"))?
    } else {
        println!(
            "\n{} {} PDA accounts in {} ({})\n",
            "▸".cyan().bold(),
            unique.len().to_string().bold(),
            project.name.cyan(),
            program_address.dimmed()
        );

        let items: Vec<String> = unique
            .iter()
            .map(|p| {
                let arg_names: Vec<&str> = p
                    .seeds
                    .iter()
                    .filter(|s| s.kind == "arg")
                    .map(|s| s.name.as_deref().unwrap_or("?"))
                    .collect();
                if arg_names.is_empty() {
                    p.account.clone()
                } else {
                    format!("{} ({})", p.account, arg_names.join(", "))
                }
            })
            .collect();

        let idx = FuzzySelect::with_theme(&ColorfulTheme::default())
            .with_prompt("Select PDA account")
            .items(&items)
            .default(0)
            .interact()?;
        unique[idx]
    };

    // Prompt for arg seed values
    let arg_seeds: Vec<&PdaSeed> = selected
        .seeds
        .iter()
        .filter(|s| s.kind == "arg" || s.kind == "account")
        .collect();

    let mut args: HashMap<String, String> = HashMap::new();
    if !arg_seeds.is_empty() {
        println!("\n{}", "Seed values".bold());
        for seed in &arg_seeds {
            let name = seed.name.as_deref().unwrap_or("value");
            let ty = if seed.kind == "account" { "publicKey" } else { seed.ty.as_deref().unwrap_or("string") };
            let theme = ColorfulTheme::default();
            let value: String = Input::with_theme(&theme)
                .with_prompt(format!("{name} ({ty})"))
                .interact_text()?;
            args.insert(name.to_string(), value);
        }
    }

    // Derive
    let spinner = indicatif::ProgressBar::new_spinner();
    spinner.set_message("Deriving PDA...");
    spinner.enable_steady_tick(std::time::Duration::from_millis(80));
    let result = client
        .derive_pda(&project.id, &selected.instruction, &selected.account, &selected.seeds, args)
        .await;
    spinner.finish_and_clear();
    let result = result?;

    println!("\n{} PDA derived!\n", "✓".green().bold());
    println!("  {:<10} {}", "Address:".bold(), result.pda.cyan().bold());
    println!("  {:<10} {}", "Bump:".bold(), result.bump.to_string().yellow());
    println!("  {:<10} {}", "Program:".bold(), result.program_id.dimmed());
    println!();
    println!("{}", "Seeds:".bold());
    for seed in &result.seeds {
        match seed.kind.as_str() {
            "const" => {
                let desc = seed.description.as_deref().unwrap_or("(const)");
                println!("  {} {:16} [{}]", "const".dimmed(), desc.green(), seed.hex.dimmed());
            }
            "arg" => {
                let name = seed.name.as_deref().unwrap_or("?");
                let val = seed.value.as_deref().unwrap_or("?");
                println!(
                    "  {} {:16} = {} [{}]",
                    "arg  ".dimmed(),
                    name.green(),
                    val.yellow(),
                    seed.hex.dimmed()
                );
            }
            other => {
                println!("  {} [{}]", other.dimmed(), seed.hex.dimmed());
            }
        }
    }
    println!();
    Ok(())
}

// ── File-mode sub-commands ────────────────────────────────────────────────────

async fn cmd_list_file(idl_path: &str) -> Result<()> {
    let parsed_idl = idl::parse_idl_file(idl_path)?;
    let instructions = idl::idl_to_instructions(&parsed_idl);

    if instructions.is_empty() {
        println!("{}", "No instructions found in IDL file.".yellow());
        return Ok(());
    }

    println!(
        "\n{} {} instructions  {}  ({})\n",
        "\u{25b8}".cyan().bold(),
        instructions.len().to_string().bold(),
        "(local IDL)".cyan(),
        parsed_idl.address.dimmed()
    );

    let name_width = instructions.iter().map(|i| i.name.len()).max().unwrap_or(10) + 2;
    for ix in &instructions {
        let args_desc: String = if ix.args.is_empty() {
            "(no args)".to_string()
        } else {
            ix.args
                .iter()
                .map(|a| format!("{}: {}", a.name, a.ty))
                .collect::<Vec<_>>()
                .join(", ")
        };
        println!(
            "  {:<width$} {}",
            ix.name.green().bold(),
            args_desc.dimmed(),
            width = name_width
        );
    }
    println!();
    Ok(())
}

async fn cmd_run_file(
    idl_path: &str,
    instruction_name: Option<&str>,
    config: &Config,
) -> Result<()> {
    let parsed_idl = idl::parse_idl_file(idl_path)?;
    let api_instructions = idl::idl_to_instructions(&parsed_idl);

    if api_instructions.is_empty() {
        bail!("No instructions found in IDL file.");
    }

    // Select instruction.
    let idx = if let Some(name) = instruction_name {
        api_instructions
            .iter()
            .position(|i| i.name == name)
            .ok_or_else(|| anyhow::anyhow!("Instruction '{name}' not found in IDL"))?
    } else {
        let items: Vec<String> = api_instructions.iter().map(|i| i.name.clone()).collect();
        FuzzySelect::with_theme(&ColorfulTheme::default())
            .with_prompt("Select instruction")
            .items(&items)
            .default(0)
            .interact()?
    };

    let ix = &api_instructions[idx];
    let ix_idl = &parsed_idl.instructions[idx];

    println!("\n{} {}\n", "Instruction:".bold(), ix.name.green().bold());

    // Collect args (reuses existing prompt logic).
    let args = collect_args(ix)?;

    // Collect accounts using IDL PDA info.
    let (accounts_full, fee_payer) =
        collect_accounts_idl(ix_idl, &args, config, &parsed_idl.address)?;

    // Summary.
    println!(
        "{}\n{}",
        "\u{2500}".repeat(40).dimmed(),
        "Summary".bold()
    );
    println!("  Instruction : {}", ix.name.cyan());
    if !args.is_empty() {
        println!("  Args        :");
        for (k, v) in &args {
            println!("    {} = {}", k.dimmed(), v);
        }
    }
    println!("  Accounts    :");
    for (name, pk, _, _) in &accounts_full {
        println!("    {} = {}", name.dimmed(), pk);
    }
    println!("  Fee payer   : {}", fee_payer);
    println!("{}", "\u{2500}".repeat(40).dimmed());
    println!();

    if !Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(format!("Build and sign transaction for '{}'?", ix.name))
        .default(true)
        .interact()?
    {
        println!("{}", "Aborted.".yellow());
        return Ok(());
    }

    // Borsh-encode args.
    let arg_bytes = idl::borsh_encode_args(&ix_idl.args, &args)?;
    let instruction_data = idl::build_instruction_data(&ix_idl.discriminator, &arg_bytes);

    // Build (pubkey, is_signer, is_writable) triples in IDL account order.
    let account_triples: Vec<(String, bool, bool)> = accounts_full
        .iter()
        .map(|(_, pk, s, w)| (pk.clone(), *s, *w))
        .collect();

    // Encode binary Solana message (zeroed blockhash placeholder).
    let unsigned_tx = solana::encode_unsigned_message(
        &fee_payer,
        &parsed_idl.address,
        &account_triples,
        &instruction_data,
    )?;

    println!("\n{} Transaction built successfully!\n", "\u{2713}".green().bold());
    let network = infer_network(config.rpc());

    if let Some(keypair_path) = &config.keypair_path {
        println!("  Keypair found : {}", keypair_path.dimmed());
        println!();
        if Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("Sign and send transaction to Solana?")
            .default(true)
            .interact()?
        {
            let spinner = indicatif::ProgressBar::new_spinner();
            spinner.set_message("Signing and sending...");
            spinner.enable_steady_tick(std::time::Duration::from_millis(80));
            let result = solana::sign_and_send(
                &unsigned_tx,
                keypair_path,
                config.rpc(),
                &fee_payer,
            )
            .await;
            spinner.finish_and_clear();
            match result {
                Ok(signature) => {
                    println!("{} Transaction confirmed!", "\u{2713}".green().bold());
                    println!("  Signature : {}", signature.cyan());
                    let explorer = explorer_url(&signature, &network);
                    println!("  Explorer  : {}", explorer.dimmed());
                }
                Err(e) => {
                    println!("{} Failed to send: {e}", "\u{2717}".red().bold());
                    println!("\n  Base58 tx (for manual signing):");
                    println!("  {}", unsigned_tx.dimmed());
                }
            }
        } else {
            print_base58_tx(&unsigned_tx);
        }
    } else {
        print_base58_tx(&unsigned_tx);
    }

    Ok(())
}

async fn cmd_pda_file(idl_path: &str, account_name: Option<&str>) -> Result<()> {
    let parsed_idl = idl::parse_idl_file(idl_path)?;

    // Collect all PDA accounts across all instructions (dedup by account name).
    let mut pda_list: Vec<(&idl::IdlInstruction, &idl::IdlAccount)> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    for ix in &parsed_idl.instructions {
        for acc in &ix.accounts {
            if acc.pda.is_some() && seen.insert(acc.name.clone()) {
                pda_list.push((ix, acc));
            }
        }
    }

    if pda_list.is_empty() {
        println!("{}", "No PDA accounts found in IDL file.".yellow());
        return Ok(());
    }

    let (ix, acc) = if let Some(name) = account_name {
        pda_list
            .iter()
            .find(|(_, a)| a.name == name)
            .copied()
            .ok_or_else(|| anyhow::anyhow!("PDA account '{name}' not found in IDL"))?
    } else {
        println!(
            "\n{} {} PDA accounts  {}  ({})\n",
            "\u{25b8}".cyan().bold(),
            pda_list.len().to_string().bold(),
            "(local IDL)".cyan(),
            parsed_idl.address.dimmed()
        );
        let items: Vec<String> = pda_list
            .iter()
            .map(|(_, a)| {
                let args: Vec<&str> = a
                    .pda
                    .as_ref()
                    .map(|p| {
                        p.seeds
                            .iter()
                            .filter(|s| s.kind != "const")
                            .filter_map(|s| s.path.as_deref())
                            .collect()
                    })
                    .unwrap_or_default();
                if args.is_empty() {
                    a.name.clone()
                } else {
                    format!("{} ({})", a.name, args.join(", "))
                }
            })
            .collect();
        let sel = FuzzySelect::with_theme(&ColorfulTheme::default())
            .with_prompt("Select PDA account")
            .items(&items)
            .default(0)
            .interact()?;
        pda_list[sel]
    };

    let pda = acc.pda.as_ref().unwrap();
    println!("\n{}", "Seed values".bold());

    let mut seed_bytes_list: Vec<Vec<u8>> = Vec::new();
    for seed in &pda.seeds {
        match seed.kind.as_str() {
            "const" => {
                let bytes = seed.value.clone().unwrap_or_default();
                let label =
                    String::from_utf8(bytes.clone()).unwrap_or_else(|_| format!("{bytes:?}"));
                println!("  {} {}", "const".dimmed(), label.green());
                seed_bytes_list.push(bytes);
            }
            "arg" => {
                let name = seed.path.as_deref().unwrap_or("value");
                let ty = ix
                    .args
                    .iter()
                    .find(|a| a.name == name)
                    .and_then(|a| match &a.ty {
                        serde_json::Value::String(s) => Some(s.clone()),
                        _ => None,
                    })
                    .unwrap_or_else(|| "string".to_string());
                let raw: String = Input::with_theme(&ColorfulTheme::default())
                    .with_prompt(format!("{} ({})", name.cyan(), ty.dimmed()))
                    .interact_text()?;
                let coerced = coerce_value(&raw, &ty);
                let bytes = idl::seed_bytes_from_value(&coerced, &ty)
                    .with_context(|| format!("Encoding seed '{name}' as {ty}"))?;
                seed_bytes_list.push(bytes);
            }
            "account" => {
                let name = seed.path.as_deref().unwrap_or("account");
                let raw: String = Input::with_theme(&ColorfulTheme::default())
                    .with_prompt(format!("{} ({})", name.cyan(), "publicKey".dimmed()))
                    .interact_text()?;
                let bytes = bs58::decode(&raw)
                    .into_vec()
                    .with_context(|| format!("Invalid pubkey for seed '{name}': {raw}"))?;
                anyhow::ensure!(bytes.len() == 32, "Pubkey must decode to 32 bytes");
                seed_bytes_list.push(bytes);
            }
            other => bail!("Unknown seed kind '{other}' in IDL"),
        }
    }

    println!();
    let (address, bump) = solana::find_program_address(&seed_bytes_list, &parsed_idl.address)?;

    println!("{} PDA derived!\n", "\u{2713}".green().bold());
    println!("  {:<10} {}", "Address:".bold(), address.cyan().bold());
    println!("  {:<10} {}", "Bump:".bold(), bump.to_string().yellow());
    println!("  {:<10} {}", "Program:".bold(), parsed_idl.address.dimmed());
    println!();
    Ok(())
}

/// Collect instruction accounts using IDL PDA definitions.
/// Auto-fills fixed-address accounts and auto-derives PDA accounts when all
/// seeds are available.  Returns `(Vec<(name, pubkey, is_signer, is_writable)>, fee_payer)`.
fn collect_accounts_idl(
    ix_idl: &idl::IdlInstruction,
    args: &HashMap<String, serde_json::Value>,
    config: &Config,
    program_id: &str,
) -> Result<(Vec<(String, String, bool, bool)>, String)> {
    let keypair_pubkey = config
        .keypair_path
        .as_deref()
        .and_then(|p| solana::pubkey_from_keypair_file(p).ok());

    // Ordered: (account_name, pubkey, is_signer, is_writable)
    let mut collected: Vec<(String, String, bool, bool)> = Vec::new();
    // name -> pubkey map for PDA seed resolution as we go
    let mut collected_map: HashMap<String, String> = HashMap::new();
    let mut fee_payer = String::new();

    println!("{}", "Accounts".bold().underline());

    for acc in &ix_idl.accounts {
        // Fixed-address account (system_program, token_program, etc.)
        if let Some(fixed_addr) = &acc.address {
            println!(
                "  {} {} {}",
                acc.name.cyan(),
                "=".dimmed(),
                fixed_addr.dimmed()
            );
            collected.push((acc.name.clone(), fixed_addr.clone(), acc.signer, acc.writable));
            collected_map.insert(acc.name.clone(), fixed_addr.clone());
            continue;
        }

        // PDA account — try to auto-derive from seeds we already have.
        if let Some(pda) = &acc.pda {
            if let Some(seed_bytes) =
                idl::resolve_pda_seeds(pda, ix_idl, &collected_map, args)
            {
                match solana::find_program_address(&seed_bytes, program_id) {
                    Ok((addr, _)) => {
                        println!(
                            "  {} {} {} {}",
                            acc.name.cyan(),
                            "=".dimmed(),
                            addr.green(),
                            "(auto-derived)".dimmed()
                        );
                        collected.push((
                            acc.name.clone(),
                            addr.clone(),
                            acc.signer,
                            acc.writable,
                        ));
                        collected_map.insert(acc.name.clone(), addr);
                        continue;
                    }
                    Err(e) => {
                        println!(
                            "  {} {}",
                            acc.name.yellow(),
                            format!("(PDA derivation failed — {e}, enter manually)").dimmed()
                        );
                    }
                }
            }
        }

        // Prompt the user.
        let tmp = InstructionAccount {
            name: acc.name.clone(),
            is_mut: acc.writable,
            is_signer: acc.signer,
            is_optional: false,
            pda: None,
        };
        let val = prompt_account(&tmp, keypair_pubkey.as_deref())?;
        if fee_payer.is_empty()
            && (acc.signer
                || acc.name.to_lowercase().contains("authority")
                || acc.name.to_lowercase().contains("payer"))
        {
            fee_payer = val.clone();
        }
        collected.push((acc.name.clone(), val.clone(), acc.signer, acc.writable));
        collected_map.insert(acc.name.clone(), val);
    }
    println!();

    if fee_payer.is_empty() {
        fee_payer = if let Some(pk) = &keypair_pubkey {
            pk.clone()
        } else {
            Input::with_theme(&ColorfulTheme::default())
                .with_prompt("Fee payer public key")
                .interact_text()?
        };
    }

    Ok((collected, fee_payer))
}
