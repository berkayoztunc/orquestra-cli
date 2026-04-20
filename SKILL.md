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

## Agent execution guide — handling interactive prompts

`orquestra run`, `orquestra pda`, and the bare `orquestra` menu all use interactive prompts (dialoguer). **Never tell the user to run the command themselves.** Instead, drive the prompts programmatically using `run_in_terminal` + `send_to_terminal`.

### How to execute an interactive command

1. Start the command with `run_in_terminal`, `mode=sync`, short timeout (e.g. `5000` ms). It will time out while waiting for input and return a terminal `id`.
2. Call `get_terminal_output` with that `id` to read the current prompt.
3. Call `send_to_terminal` with the `id` to answer the prompt — **one answer per call**, exactly matching what the prompt expects.
4. After each send, call `get_terminal_output` again to read the next prompt.
5. Repeat until the command exits (output shows a signature/explorer link or an error).

### Prompt sequence for `orquestra run <instruction>`

Always pass the instruction name directly (e.g. `orquestra run initialize`) to **skip** the FuzzySelect menu — the menu requires arrow keys which cannot be automated. With the name provided, prompts appear in this order:

| Step | Prompt looks like | What to send |
|------|-------------------|--------------|
| 1 | `{arg_name} ({type}):` — one per instruction arg | The value (e.g. `42`), then `\n` |
| 2 | `{acc_name} [mut, signer]:` — one per account | The public key address, then `\n`. **Signer accounts auto-fill with keypair pubkey — send just `\n` to accept.** |
| 3 | `Build transaction for '…'? [Y/n]` | `\n` to confirm (default Yes), or `n\n` to abort |
| 4 | `Sign and send transaction to Solana? [Y/n]` | `\n` to confirm (default Yes), or `n\n` to skip and print base58 tx instead |

If the instruction has **no args**, step 1 is skipped entirely.

### Prompt sequence for `orquestra pda <account-name>`

| Step | Prompt looks like | What to send |
|------|-------------------|--------------|
| 1 | `{seed_name} ({type}):` — one per seed | The seed value, then `\n` |

The derived address is printed to stdout after all seeds are entered.

### Example (agent drives `orquestra run <instruction>`)

```
run_in_terminal("orquestra run <instruction>", mode=sync, timeout=5000)
# → returns terminal id, output shows first account/arg prompt
send_to_terminal(id, "<VALUE>\n")   # answer first prompt
# → get_terminal_output shows: "<signer_account> [signer]: <KEYPAIR_PUBKEY>"
send_to_terminal(id, "\n")          # accept default signer
# → get_terminal_output shows: "Build transaction for '<instruction>'? [Y/n]"
send_to_terminal(id, "\n")          # confirm build
# → get_terminal_output shows: "Sign and send transaction to Solana? [Y/n]"
send_to_terminal(id, "\n")          # confirm send
# → get_terminal_output shows signature + explorer link
```

### Do not do this

- Do **not** tell the user "this requires an interactive terminal, please run it yourself".
- Do **not** use bare `orquestra run` without an instruction name — the FuzzySelect cannot be driven non-interactively.
- Do **not** send multiple answers in a single `send_to_terminal` call.
