# 🛠️ Welder PRD (Product Requirements Document)

**Status:** Draft / Initializing 🏗️  
**Version:** 0.2.0 (PRD revision; MVP still targets v0.1)  
**Author:** Kaf & Athena

## 🎯 Overview
**Welder** is a high-speed CLI tool designed to bridge the gap between **raw pixel art** and **ship-ready asset packs**. It automates the technical drudgery of resizing, packing, previewing, packaging, and publishing to platforms like itch.io.

## 🚀 Core Objectives
1. **Automate the Technical Pass:** Batch resize and package sprites with deterministic outputs.
2. **Generate Previews:** Auto-create sprite sheets and grid previews suitable for store listings.
3. **Streamline Publishing:** Wrap metadata + package + upload to itch.io (via Butler) in one command.
4. **Consistency:** Ensure every asset pack follows a high-quality, branded structure.

## ✅ Non-Goals (MVP)
- Full Aseprite project parsing/export (PNG-only for v0.1; Aseprite support later).
- Complex recoloring / palette swapping (schema reserved, implementation optional in v0.1).
- GUI app (CLI only).
- Replacing Butler (Welder shells out to it; does not reimplement itch.io upload).

---

## 🧭 Primary User Flow (Golden Path)
1. `welder init` — scaffold config + folders
2. `welder doctor` — verify dependencies (esp. Butler)
3. `welder build` — generate exports (1x/2x/4x)
4. `welder preview` — generate store images (sheet/grid) + watermark previews only
5. `welder package` — create versioned zip
6. `welder publish` — run `butler push` (supports `--dry-run`)

---

## 🛠️ CLI Surface (v0.1)

### Global flags
- `-C, --cwd <dir>`: operate as if run in that directory
- `--config <path>`: config path (default `welder.toml`)
- `-v, --verbose` / `-q, --quiet`

### Commands

#### `welder init`
Scaffold `welder.toml` and project folders.
- Flags:
  - `--name <pack-name>`
  - `--author <name>`
  - `--brand <brand>`
  - `--input <dir>` (default `src`)
  - `--yes` (no prompts)

#### `welder doctor`
Verify environment (config validity, paths exist, dependencies).
- Checks:
  - `butler` available (`butler --version`)
  - `welder.toml` parse + required fields
  - expected folders exist (or are creatable)
- Flags:
  - `--butler` (only check Butler)

#### `welder build`
Perform the technical pass: export/resize into `dist/exports/`.
- MVP behavior:
  - PNG inputs only
  - nearest-neighbor scaling for pixel art
  - deterministic ordering
- Flags:
  - `--profile <name>` (default `default`)
  - `--res <1,2,4>` (override resolutions)
  - `--clean` (wipe `dist/` first)
  - `--dry-run`

#### `welder preview`
Generate store previews into `dist/previews/`.
- Hard rule: **watermark applies to previews only**, never exports.
- Flags:
  - `--profile <name>`
  - `--style sheet|grid|both` (default `both`)
  - `--dry-run`

#### `welder package`
Create `dist/package/<slug>-<semver>.zip`.
- Flags:
  - `--profile <name>`
  - `--out <path>`
  - `--include-previews` (default off in v0.1)

#### `welder publish`
Package + push to itch.io via Butler.
- Flags:
  - `--profile <name>`
  - `--channel <channel>` (override)
  - `--dry-run` (print butler command, do not execute)
  - `--yes`

---

## 🗂️ Config: `welder.toml` (Schema v1)
MVP uses TOML (Rust-friendly). YAML may be considered later, but v0.1 is TOML-only.

```toml
version = 1

[pack]
name = "Forest Tiles"
slug = "forest-tiles"          # dist naming + itch slug by default
author = "iamkaf"
brand = "iamkaf"
license = "CC0-1.0"            # optional
semver = "0.1.0"

[paths]
input = "src"
dist = "dist"
previews = "dist/previews"
exports = "dist/exports"
sheets  = "dist/sheets"
package = "dist/package"

[inputs]
include = ["**/*.png"]
exclude = ["**/_wip/**", "**/.trash/**"]

[build]
resolutions = [1, 2, 4]
filter = "nearest"
trim_transparent = true

[preview]
styles = ["sheet", "grid"]
background = "#141414"
scale = 2

[preview.watermark]
enabled = true
text = "iamkaf"
opacity = 0.12
position = "bottom-right"      # tl,tr,bl,br,center
margin_px = 12

[sheet]
max_width = 2048
max_height = 2048
padding_px = 2
sort = "name"                  # stable output

[grid]
cell_px = 64
padding_px = 8
columns = 8

[metadata]
readme_template = "templates/README.md.tmpl"   # optional
itch_template = "templates/ITCH.md.tmpl"       # optional

[publish.itch]
enabled = true
user = "iamkaf"
project = "forest-tiles"
channel = "default"
butler_bin = "butler"
```

---

## 📦 Output Contract (Deterministic)
Welder outputs are part of the product contract; consumers can rely on paths and naming.

After `welder build`:
- `dist/exports/1x/**.png`
- `dist/exports/2x/**.png`
- `dist/exports/4x/**.png`

After `welder preview`:
- `dist/previews/sheet.png`
- `dist/previews/grid.png`
- (optional) `dist/previews/thumb.png`

After `welder package`:
- `dist/package/<slug>-<semver>.zip`

**Determinism requirements:**
- Stable file ordering (configurable: `sheet.sort = "name"` etc.)
- Identical inputs + config ⇒ identical outputs (byte-identical where feasible)

---

## 🧱 Architecture (Modules)
Map legacy concepts to Welder’s subcommands.

### 1) Processing Core (`pixel` legacy)
- Batch resize/export
- Folder packaging
- (Later) palette swaps

### 2) Presentation Layer (`easel` legacy)
- Sheet generator
- Grid preview generator
- (Later) thumbnail generator + templates

### 3) Delivery Engine (Butler wrapper)
- Dependency + manifest checks
- `butler push` orchestration (dry-run supported)

---

## 📋 Technical Specs
- **Language:** Rust
- **CLI:** `clap`
- **Image processing:** `image` crate
- **Config:** `toml`
- **Itch.io:** external `butler` dependency

---

## 🗺️ Roadmap + Acceptance Criteria

### Phase 1: Skeleton
**Deliverables**
- `welder init` creates `welder.toml` + directory skeleton
- `welder --help` stable, documented commands
- `welder doctor` validates config + checks `butler --version`

**Acceptance**
- Running init + doctor on a fresh folder completes with clear, actionable output

### Phase 2: Build + Preview
**Deliverables**
- `welder build` generates 1x/2x/4x exports (PNG-only), nearest scaling
- `welder preview` generates `sheet.png` + `grid.png`
- Watermark applied to previews only

**Acceptance**
- Given a sample project, outputs match the output contract and reruns are deterministic

### Phase 3: Package + Publish
**Deliverables**
- `welder package` produces versioned zip from exports
- `welder publish` runs `butler push` with configurable `user/project/channel`
- `--dry-run` prints the exact butler command and planned file paths

**Acceptance**
- Dry-run produces correct command; real publish succeeds when credentials are configured

---

## ⚠️ Risks / Open Questions
- Palette swapping approach (index-based vs RGB nearest-match) and input palette format
- Sheet packing algorithm constraints (max size, padding, atlas metadata)
- Whether previews should be included in the published channel or a separate channel
