# Welder

Welder is a fast CLI that turns **raw pixel art** into **ship-ready asset packs**.

It automates the boring parts:

- Deterministic exports (1x/2x/4x)
- Store previews (sprite sheet + grid)
- Packaging + publishing to itch.io via **Butler**

## Status

Early skeleton. See `PRD.md` for the roadmap and CLI contract.

## Quickstart

```bash
# in an asset pack folder
welder init --yes
welder doctor
welder build
welder preview
welder package
welder publish --dry-run
```

## Config

Welder uses `welder.toml` (TOML-only for v0.1). A starter file is included in this repo.

## Development

This repository is intended to be built with Rust (edition 2021).

Useful local checks:

- `cargo fmt`
- `cargo check`

If you don’t have a Rust toolchain installed:

- https://rustup.rs

## License

MIT (tooling code). Your art packs’ licenses are independent and set in `welder.toml`.
