---
name: orquestra-cli
description: Work with the orquestra-cli Rust project to add commands, update config, improve API/file flows, and debug Solana transaction UX.
argument-hint: What do you want to build or fix in orquestra-cli?
---

# orquestra-cli Development Skill

Use this skill when an agent must modify, debug, or extend this repository.

## When to use

- Add or change CLI subcommands in clap/router flow.
- Update config fields or defaults.
- Modify interactive prompts and command UX.
- Work on API mode or local IDL file mode behavior.
- Debug transaction build/sign/send and PDA derivation.
- Prepare release-related version/changelog updates.

## Repository map

| File | Responsibility |
|------|---------------|
| `src/main.rs` | Entry point and command routing |
| `src/cli.rs` | `clap` command/arg definitions |
| `src/config.rs` | Config schema, load/save/merge, display |
| `src/api.rs` | API client methods (`resolve_project_id`, `list_instructions`, `list_pda_accounts`) |
| `src/interactive.rs` | End-user command flows and prompts |
| `src/idl.rs` | Local Solana/Anchor IDL JSON parsing |
| `src/solana.rs` | Transaction build/sign/send, keypair load, PDA derivation |

## Constraints and gotchas

- Config path on macOS: `~/Library/Application Support/orquestra/config.toml`.
- Important config fields: `project_id`, `api_key`, `rpc_url`, `keypair_path`, `api_base_url`, `idl_path`.
- Normalize string fields through config merge helpers; keypair path may include trailing spaces from user input.
- If `idl_path` is set, command flows should bypass API and use file mode.

## Command surface

```bash
orquestra
orquestra list
orquestra run [instruction]
orquestra pda [account]
orquestra sign <base58-tx>
orquestra search [query]
orquestra config set ...
orquestra config show
orquestra config reset
```

## Work pattern for agents

1. Read related files in `src/` before editing.
2. Keep behavior parity between API mode and local IDL file mode where expected.
3. Use `anyhow::Result`, user-readable `bail!`, and `.context()` for IO/parse failures.
4. Preserve current prompt style (`dialoguer`, `ColorfulTheme`, `FuzzySelect` where applicable).
5. Validate with `cargo run -- <args>` or `cargo build` after changes.

## Typical edits

### Add subcommand

1. Add variant/args in `src/cli.rs`.
2. Add router arm in `src/main.rs`.
3. Implement command handler in `src/interactive.rs`.
4. Add menu entry in `cmd_menu` when needed.
5. Extend `src/api.rs` if remote data is required.

### Add config field

1. Add field to `Config` in `src/config.rs`.
2. Add CLI flag in `ConfigSetArgs` in `src/cli.rs`.
3. Merge field in `Config::merge()`.
4. Show field in `Config::display()` when relevant.

## Build and release

```bash
cargo run -- <args>
cargo build --release
cargo build --release --target aarch64-apple-darwin
```

Release flow uses `.github/workflows/release.yml` and version tags (`vX.Y.Z`).
