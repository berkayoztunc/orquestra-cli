# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [0.2.5] - 2026-04-21

### Added

- Non-interactive execution options for CLI commands.

### Changed

- Refined and clarified `SKILL.md` usage guidance and agent execution instructions.

---

## [0.2.4] - 2026-04-21

### Fixed

- Updated Homebrew formula checksums for binary integrity verification.

---

## [0.2.1] - 2026-04-07

### Added

- **Sign tx** command — new interactive menu option (`Sign tx`) and `cmd_sign_tx` function to accept a base58-encoded serialized transaction, sign it with the configured keypair, and broadcast it to the network.
- `serialized_transaction` field support in `BuildResponse` — the CLI now prefers `serializedTransaction` from the Orquestra API response when available, falling back to `transaction`.

### Fixed

- `extract_message_bytes` now tries the wire-transaction format (strip compact-u16 signature count + signatures) before the raw-message fallback. This prevents a false positive where a 1-signature wire transaction was mistakenly treated as a raw message, producing a garbage blockhash offset.

---

## [0.2.0] - 2026-04-06

### Added

- **Local IDL file mode** — the CLI can now operate entirely without an Orquestra account or API key by pointing it at a local Solana/Anchor IDL JSON file (`orquestra config set --idl <path>`).
- `--idl` flag for `orquestra config set` to configure the IDL file path.
- `idl_path` field in `config.toml` and `orquestra config show` output.
- `idl_path` prompt in `orquestra config reset` interactive setup.
- `src/idl.rs` — new module for IDL parsing, Borsh argument encoding, PDA seed resolution, and instruction data building.
  - Supported Borsh types: `string`, `u8`–`u128`, `i8`–`i128`, `bool`, `pubkey`.
  - Discriminator + Borsh payload assembly (`build_instruction_data`).
  - PDA seed resolution from collected args and accounts (`resolve_pda_seeds`).
- `solana::find_program_address` — local SHA256-based PDA derivation with `curve25519-dalek` on-curve rejection check (bump 255 → 0).
- `solana::encode_unsigned_message` — builds a binary Solana legacy message with a zeroed blockhash placeholder; `sign_and_send` patches the blockhash and signs as usual.
- In file mode, `orquestra list` shows all instructions and their argument types from the IDL without any network call.
- In file mode, `orquestra run` collects args with Borsh encoding, auto-fills fixed-address accounts (system_program, token_program, etc.), auto-derives PDA accounts whose seeds are fully resolvable, and signs/sends using the existing RPC path.
- In file mode, `orquestra pda` lists PDA accounts from the IDL and derives addresses locally.

### Changed

- README: updated intro, features list, and setup section to document both API mode and local IDL file mode.

---

## [0.1.1] - 2026-04-04

### Fixed

- Robust transaction decoding: added base64 (standard and URL-safe) fallback in addition to base58 when decoding the unsigned transaction returned by the build API.
- Binary message reconstruction from the Orquestra JSON transaction format (`TxJson`) with canonical Solana account ordering and fresh blockhash injection.
- Blockhash patching fallback path for raw binary wire transactions.
- Config `keypair_path` values are now trimmed of leading/trailing whitespace before file reads.

---

## [0.1.0] - 2026-04-03

### Added

- Initial release.
- `orquestra list` — fetches and displays all instructions for a configured Solana program via the Orquestra API.
- `orquestra run [INSTRUCTION]` — interactive fuzzy-select instruction runner with per-arg prompts and type coercion.
- `orquestra pda [ACCOUNT]` — interactive PDA derivation via the Orquestra API.
- Auto-fill signer accounts from a configured local keypair.
- Sign & send — ed25519 signing with a local Solana keypair JSON file and broadcast via JSON-RPC `sendTransaction`.
- Keypair-free mode — prints the base58 unsigned transaction for manual wallet signing.
- Interactive top-level menu (`orquestra` with no arguments).
- `orquestra config set / show / reset` for persistent TOML-based configuration.
- Homebrew tap formula and GitHub Actions release pipeline for macOS arm64/x86_64 and Linux x86_64/arm64 binaries.

[0.2.5]: https://github.com/berkayoztunc/orquestra-cli/compare/v0.2.4...v0.2.5
[0.2.4]: https://github.com/berkayoztunc/orquestra-cli/compare/v0.2.2...v0.2.4
[0.2.2]: https://github.com/berkayoztunc/orquestra-cli/compare/v0.2.1...v0.2.2
[0.2.0]: https://github.com/berkayoztunc/orquestra-cli/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/berkayoztunc/orquestra-cli/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/berkayoztunc/orquestra-cli/releases/tag/v0.1.0
