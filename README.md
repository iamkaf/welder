# Welder

Welder is a fast CLI that turns **raw pixel art** into **ship-ready asset packs**.

It automates the boring parts:

- Deterministic exports (1x/2x/4x)
- Store previews (sprite sheet + grid)
- Packaging + publishing to itch.io via **Butler**

## Status

Shippable MVP. See `PRD.md` for the CLI contract + roadmap.

## Install

```sh
cargo install --git https://github.com/iamkaf/welder
```

## Usage

```sh
# in an asset pack folder
welder init --yes
welder doctor
welder build
welder preview
welder package
welder publish --dry-run
```

## Config

Welder uses `welder.toml` (TOML-only for v0.1).

## Notes

- `welder publish` shells out to **butler**. If butler isn’t installed, `--dry-run` still works and non-dry-run will error with install instructions.


## License

MIT (tooling code). Your art packs’ licenses are independent and set in `welder.toml`.
