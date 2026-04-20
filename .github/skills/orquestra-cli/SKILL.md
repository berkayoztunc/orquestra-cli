---
name: orquestra-cli
description: 'Work with the orquestra-cli Rust project: add commands, modify config, implement features, release new versions, and debug issues. Use when: adding a new CLI subcommand, changing config fields, updating API logic, modifying interactive prompts, deriving PDAs, building or releasing the binary, debugging keypair or config issues.'
argument-hint: What do you want to build or fix?
---

# orquestra-cli Development Skill

A Rust CLI that turns Solana program instructions (via Anchor IDL) into interactive prompts. Users configure a program ID + API key (orquestra.dev) **or** point at a local IDL JSON file for fully offline use.

## Source Layout

| File | Responsibility |
|------|---------------|
| `src/main.rs` | Entry point — parses CLI args, routes to `interactive::cmd_*` functions |
| `src/cli.rs` | `clap` structs: `Commands`, `ConfigAction`, `ConfigSetArgs` |
| `src/config.rs` | `Config` struct, `load/save/merge/display`, config path helpers |
| `src/api.rs` | `ApiClient` — `resolve_project_id`, `list_instructions`, `list_pda_accounts` |
| `src/interactive.rs` | All user-facing flows: menu, list, run, pda, sign, search, config reset |
| `src/idl.rs` | Parse local Solana IDL JSON files into `Instruction`/`PdaAccount` types |
| `src/solana.rs` | Build, sign, and send transactions; keypair loading; PDA derivation |

## Config

- **Path (macOS):** `~/Library/Application Support/orquestra/config.toml`
- **Fields:** `project_id`, `api_key`, `rpc_url`, `keypair_path`, `api_base_url`, `idl_path`
- **Defaults:** `api_base_url = "https://api.orquestra.dev"`, `rpc_url = "https://api.mainnet-beta.solana.com"`
- **Gotcha:** All string fields are normalized through `normalize_optional()` on `merge()` — this trims whitespace. Keypair paths from interactive input may contain trailing spaces; always use `normalize_optional` before file reads.
- `Config::require_project_id()` returns an error when `project_id` is unset and `idl_path` is also unset.

## CLI Commands

```
orquestra                          # interactive top-level menu
orquestra list                     # print all instructions
orquestra run [instruction]        # interactive run (fuzzy-select if omitted)
orquestra pda [account]            # derive PDA (fuzzy-select if omitted)
orquestra sign <base58-tx>         # sign & send a serialized transaction
orquestra search [query]           # search orquestra.dev programs
orquestra config set --project-id <ADDR> --api-key <KEY> --keypair <PATH>
orquestra config show              # masked API key
orquestra config reset             # interactive reset
```

## Local IDL File Mode

When `idl_path` is set in config (or `--idl` flag), all commands bypass the API and parse the IDL JSON directly via `src/idl.rs`. No `project_id` or `api_key` required.

## Adding a New Subcommand

1. **`src/cli.rs`** — add a variant to `Commands` (and args struct if needed).
2. **`src/main.rs`** — add a `Some(Commands::NewCmd { .. })` match arm, load config, call new interactive fn.
3. **`src/interactive.rs`** — implement `pub async fn cmd_new(config: &Config, ...) -> Result<()>`.
4. **`src/interactive.rs` `cmd_menu`** — add an option string and a match arm if it belongs in the interactive menu.
5. If it needs API data, add a method to `ApiClient` in `src/api.rs`.

## Adding a Config Field

1. Add `pub field: Option<String>` to `Config` in `src/config.rs`.
2. Add a `--flag` to `ConfigSetArgs` in `src/cli.rs`.
3. Add a `merge` branch in `Config::merge()`.
4. Update `Config::display()` if it should appear in `orquestra config show`.

## Build Commands

```bash
# Development build + run
cargo run -- <args>

# Release build (native)
cargo build --release
# Binary at: target/release/orquestra

# Cross-compile for macOS arm64
cargo build --release --target aarch64-apple-darwin
# Binary at: target/aarch64-apple-darwin/release/orquestra
```

## Release Process

Releases are fully automated via `.github/workflows/release.yml`:

1. Bump version in `Cargo.toml` and update `CHANGELOG.md`.
2. Commit changes: `git add Cargo.toml CHANGELOG.md && git commit -m "chore: release vX.Y.Z"`
3. Tag and push: `git tag vX.Y.Z && git push origin vX.Y.Z`
4. CI builds binaries for 4 targets (macOS arm64/x86_64, Linux amd64/arm64) and creates a GitHub release.
5. Update `Formula/orquestra-cli.rb` SHA256 checksums after binaries are published.

## Key Patterns

- **Spinners:** Use `indicatif::ProgressBar::new_spinner()` with `enable_steady_tick(80ms)` around async API calls, then `finish_and_clear()`.
- **Interactive prompts:** Use `dialoguer` with `ColorfulTheme`. `FuzzySelect` for lists, `Input` for free text, `Confirm` for yes/no.
- **Error handling:** `anyhow::Result` everywhere; `bail!` for user-facing errors; `.context()` on IO/parse ops.
- **Colors:** `colored` crate — `"text".green().bold()` for success, `.yellow()` for warnings, `.cyan().bold()` for headers, `.dimmed()` for secondary info.
- **File mode branching:** At the top of each `cmd_*` function, check `if let Some(idl_path) = &config.idl_path` and call the `_file` variant instead of the API variant.

## Common Debug Points

- Config not loading → check `~/Library/Application Support/orquestra/config.toml` exists and is valid TOML.
- Keypair error → path has trailing space; `normalize_optional` trims it on write but old configs may not.
- API 401 → `api_key` is unset or wrong; `config show` to verify (masked).
- PDA derivation fails → seed type mismatch; check `src/solana.rs` seed encoding logic.
