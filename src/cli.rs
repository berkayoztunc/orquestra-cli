use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "orquestra",
    about = "Interact with Solana programs via orquestra.dev",
    version,
    propagate_version = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// List all instructions for the configured program
    List,

    /// Interactively run an instruction (select from list or specify name)
    Run {
        /// Instruction name to run directly (skips selection menu)
        instruction: Option<String>,
    },

    /// Find and derive program-derived addresses (PDAs)
    Pda {
        /// PDA account name to derive directly (skips selection menu)
        account: Option<String>,
    },

    /// Sign and send a base58-encoded serialized transaction
    Sign {
        /// Base58-encoded serialized transaction
        tx: String,
    },

    /// Search for programs on orquestra
    Search {
        /// Search query (prompts interactively if omitted)
        query: Option<String>,
    },

    /// Manage CLI configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Subcommand)]
pub enum ConfigAction {
    /// Set one or more config values
    Set(ConfigSetArgs),

    /// Show current config (API key is masked)
    Show,

    /// Interactively reset config values to defaults or clear them
    Reset,
}

#[derive(Args)]
pub struct ConfigSetArgs {
    /// Orquestra project ID (Solana program address, e.g. BUYuxRf...)
    #[arg(long)]
    pub project_id: Option<String>,

    /// Orquestra API key (X-API-Key header)
    #[arg(long)]
    pub api_key: Option<String>,

    /// Solana RPC URL
    #[arg(long)]
    pub rpc: Option<String>,

    /// Path to Solana keypair JSON file
    #[arg(long)]
    pub keypair: Option<String>,

    /// Orquestra API base URL (default: https://api.orquestra.build)
    #[arg(long)]
    pub api_base: Option<String>,

    /// Path to a local Solana IDL JSON file (enables offline/file mode)
    #[arg(long)]
    pub idl: Option<String>,
}
