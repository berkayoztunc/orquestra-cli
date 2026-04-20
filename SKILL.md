---
name: orquestra-cli
description: Use the orquestra CLI to configure a Solana program, list/run instructions, derive PDAs, sign/simulate transactions, and search programs.
argument-hint: What do you want to do with the orquestra CLI?
---

# orquestra CLI Skill

Use this skill when an agent needs to operate the `orquestra` CLI.

## Quick Setup

```bash
# API mode
orquestra config set --project-id <PROGRAM_ID>
orquestra config set --api-key <API_KEY>
orquestra config set --keypair ~/.config/solana/id.json

# Optional
orquestra config set --rpc https://api.mainnet-beta.solana.com
orquestra config set --api-base https://api.orquestra.dev

# Verify
orquestra config show
```

Config path (macOS): `~/Library/Application Support/orquestra/config.toml`

## Local IDL Mode (No API)

```bash
orquestra config set --idl ./path/to/program.json
orquestra config show
```

When `idl_path` is set, instruction/PDA data is loaded from the local IDL file.

## Core Commands

```bash
orquestra list
orquestra run [instruction]
orquestra pda [account]
orquestra sign <BASE58_TX>
orquestra simulate [BASE58_TX]
orquestra tx [SIGNATURE]
orquestra search [query]
orquestra idl fetch [PROGRAM_ID] [-o output.json]
orquestra config show
orquestra config reset
```

## Preferred Agent Pattern (Non-Interactive)

Use direct flags to avoid prompt loops:

```bash
orquestra run <instruction> \
  --arg <name>=<value> \
  --account <name>=<address> \
  --yes

orquestra pda <account> \
  --seed <name>=<value>
```

Notes:
- Repeat `--arg`, `--account`, and `--seed` as needed.
- Missing values still fall back to prompts.
- Signer accounts are auto-filled from configured keypair when available.

## Common Fixes

- `project_id not set`:
  - Run `orquestra config set --project-id <PROGRAM_ID>`
  - Or set `--idl` for local file mode.
- `No keypair configured`:
  - Run `orquestra config set --keypair <PATH>`.
- Bad key/value flag input:
  - Use exact `KEY=VALUE` format for `--arg`, `--account`, `--seed`.
