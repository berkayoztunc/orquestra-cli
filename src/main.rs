mod api;
mod cli;
mod config;
mod idl;
mod interactive;
mod solana;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Commands, ConfigAction, IdlAction};
use colored::Colorize;
use config::Config;
use interactive::RunOpts;
use std::collections::HashMap;

fn parse_kv_pairs(pairs: &[String]) -> Result<HashMap<String, String>> {
    let mut map = HashMap::new();
    for pair in pairs {
        let (k, v) = pair.split_once('=').ok_or_else(|| {
            anyhow::anyhow!("Invalid key=value pair: '{pair}'. Expected format: KEY=VALUE")
        })?;
        map.insert(k.trim().to_string(), v.trim().to_string());
    }
    Ok(map)
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        None => {
            let config = Config::load()?;
            interactive::cmd_menu(&config).await?;
        }

        Some(Commands::List) => {
            let config = Config::load()?;
            interactive::cmd_list(&config).await?;
        }

        Some(Commands::Run { instruction, args, accounts, yes }) => {
            let config = Config::load()?;
            let opts = RunOpts {
                args: parse_kv_pairs(&args)?,
                accounts: parse_kv_pairs(&accounts)?,
                yes,
            };
            interactive::cmd_run(&config, instruction.as_deref(), opts).await?;
        }

        Some(Commands::Pda { account, seeds }) => {
            let config = Config::load()?;
            let seeds_map = parse_kv_pairs(&seeds)?;
            interactive::cmd_pda(&config, account.as_deref(), seeds_map).await?;
        }

        Some(Commands::Sign { tx }) => {
            let config = Config::load()?;
            interactive::cmd_sign_tx_direct(&config, &tx).await?;
        }

        Some(Commands::Search { query, yes }) => {
            let config = Config::load()?;
            interactive::cmd_search(&config, query.as_deref(), yes).await?;
        }

        Some(Commands::Simulate { tx }) => {
            let config = Config::load()?;
            interactive::cmd_simulate(&config, tx.as_deref()).await?;
        }

        Some(Commands::Tx { signature }) => {
            let config = Config::load()?;
            interactive::cmd_tx(&config, signature.as_deref()).await?;
        }

        Some(Commands::Idl { action }) => match action {
            IdlAction::Fetch { program_id, output } => {
                let config = Config::load()?;
                interactive::cmd_idl_fetch(&config, program_id.as_deref(), output.as_deref()).await?;
            }
        },

        Some(Commands::Config { action }) => match action {
            ConfigAction::Set(args) => {
                let mut config = Config::load()?;
                config.merge(Config {
                    project_id: args.project_id,
                    api_key: args.api_key,
                    rpc_url: args.rpc,
                    keypair_path: args.keypair,
                    api_base_url: args.api_base,
                    idl_path: args.idl,
                });
                config.save()?;
                println!("{} Config saved.", "✓".green().bold());
            }

            ConfigAction::Show => {
                let config = Config::load()?;
                println!("{}", config.display());
            }

            ConfigAction::Reset => {
                interactive::cmd_config_reset().await?;
            }
        },
    }

    Ok(())
}
