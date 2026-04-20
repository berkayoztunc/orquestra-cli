---
name: orquestra-cli
description: Use the orquestra CLI to interact with Solana programs — configure project, list/run instructions, derive PDAs, sign/simulate transactions, look up tx details, and search programs. Use when a user asks to run a Solana instruction, derive a PDA, sign or simulate a transaction, look up a transaction, search orquestra programs, or configure the CLI.
argument-hint: What do you want to do with the orquestra CLI?
---

# orquestra CLI Usage Skill

Use this skill when an AI agent needs to operate the `orquestra` CLI to interact with Solana programs.

## Install

```bash
curl -fsSL https://raw.githubusercontent.com/berkayoztunc/orquestra-cli/master/install.sh | bash
```

## Configuration

Config is stored at `~/Library/Application Support/orquestra/config.toml` (macOS).

```bash
# Set project (Solana program address)
orquestra config set --project-id <PROGRAM_ADDRESS>

# Set API key (from orquestra.dev)
orquestra config set --api-key <YOUR_API_KEY>

# Set Solana keypair for signing
orquestra config set --keypair ~/.config/solana/id.json

# Set RPC endpoint (default: mainnet-beta)
orquestra config set --rpc https://api.mainnet-beta.solana.com

# Use a local IDL file instead of the API (offline/file mode)
orquestra config set --idl ./path/to/program.json

# Show current config (API key masked)
orquestra config show

# Reset config interactively
orquestra config reset
```

**Required fields before running instructions:** `project_id` (or `idl_path`), `keypair` (for signing).

## Commands

### List instructions
```bash
orquestra list
```
Lists all instructions available in the configured program.

### Run an instruction
```bash
orquestra run                  # interactive fuzzy-select
orquestra run <instruction>    # run directly by name, e.g. orquestra run initialize
```
Prompts for args and accounts, shows a summary, builds the transaction, then signs and sends if a keypair is configured. If no keypair, outputs the base58 serialized transaction.

### Derive a PDA
```bash
orquestra pda                  # interactive select
orquestra pda <account-name>   # derive by account name directly
```
Prompts for seeds and derives the program-derived address.

### Sign and send a transaction
```bash
orquestra sign <BASE58_TX>
```
Signs the provided base58-encoded serialized transaction with the configured keypair and broadcasts it.

### Simulate a transaction
```bash
orquestra simulate             # prompts for tx
orquestra simulate <BASE58_TX> # simulate directly
```
Runs a dry-run simulation — shows logs and compute units without sending.

### Look up a transaction
```bash
orquestra tx                   # prompts for signature
orquestra tx <SIGNATURE>       # look up directly
```
Fetches and displays transaction details, status, and logs.

### Search programs
```bash
orquestra search               # prompts for query
orquestra search <query>       # search directly, e.g. orquestra search "token swap"
```
Searches programs indexed on orquestra.dev.

### Fetch IDL
```bash
orquestra idl fetch                             # uses config project_id
orquestra idl fetch <PROGRAM_ID>                # override program
orquestra idl fetch <PROGRAM_ID> -o ./idl.json  # save to custom path
```

### Interactive menu
```bash
orquestra   # no subcommand — launches interactive menu
```

## Two modes

| Mode | When | Behavior |
|------|------|----------|
| **API mode** | `project_id` set, no `idl_path` | Fetches instructions/PDAs from orquestra.dev API |
| **File mode** | `idl_path` set | Parses local IDL JSON, builds tx locally — no API network call |

## Common workflows

### Run an instruction end-to-end
```bash
orquestra config set --project-id <ADDR> --keypair ~/.config/solana/id.json
orquestra run initialize
# → prompts for args/accounts → builds tx → signs & sends → prints signature + explorer link
```

### Offline with local IDL
```bash
orquestra config set --idl ./my_program.json --keypair ~/.config/solana/id.json
orquestra run
```

### Build tx without sending (no keypair)
```bash
orquestra config set --project-id <ADDR>
orquestra run transfer
# → no keypair configured → prints base58 tx for external signing
# then sign manually:
orquestra sign <BASE58_TX>
```

### Simulate before sending
```bash
orquestra simulate <BASE58_TX>
```

## Tips for agents

- Always run `orquestra config show` first to confirm required fields are set.
- If a command fails with "project_id not set", run `orquestra config set --project-id <ADDR>`.
- Keypair path must not have trailing spaces; use `orquestra config set --keypair <path>` to normalize.
- Explorer links are printed automatically after confirmed sends (Solana Explorer, mainnet or devnet based on RPC).
