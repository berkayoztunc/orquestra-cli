mod api;
mod cli;
mod config;
mod idl;
mod interactive;
mod solana;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Commands, ConfigAction};
use colored::Colorize;
use config::Config;

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

        Some(Commands::Run { instruction }) => {
            let config = Config::load()?;
            interactive::cmd_run(&config, instruction.as_deref()).await?;
        }

        Some(Commands::Pda { account }) => {
            let config = Config::load()?;
            interactive::cmd_pda(&config, account.as_deref()).await?;
        }

        Some(Commands::Sign { tx }) => {
            let config = Config::load()?;
            interactive::cmd_sign_tx_direct(&config, &tx).await?;
        }

        Some(Commands::Search { query }) => {
            let config = Config::load()?;
            interactive::cmd_search(&config, query.as_deref()).await?;
        }

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
