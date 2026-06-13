# 3MF Merger

`3mf-merger` is a command-line tool for combining multiple Bambu Studio `.3mf`
project files into one merged `.3mf` package.

It preserves the source models, plate metadata, thumbnails, object settings,
filament mappings, and Bambu-specific project configuration closely enough for
merged projects to load back into Bambu Studio.

Repository: <https://github.com/matbeedotcom/3mf-merger/>

## Status

This is early software built around real Bambu Studio project files. It is most
useful for merging prepared `.3mf` projects that already load correctly in
Bambu Studio.

Before printing a merged file, open it in Bambu Studio and verify:

- plates are in the expected positions
- per-object colors/extruders are correct
- filament presets are correct
- printer/process settings are safe for your machine

## Features

- Merge two or more `.3mf` files into a single output package.
- Preserve Bambu plate metadata, thumbnails, top/pick images, and plate order.
- Relayout merged plates into Bambu-style grids:
  - 1 plate: `1x1`
  - 2 plates: `2x1`
  - 3-9 plates: `3xN`
  - 10-16 plates: `4xN`
  - 17-25 plates: `5xN`
  - 26-36 plates: `6xN`
- Rewrite object IDs, model part paths, production UUIDs, plate references, and
  filament references so inputs can coexist in one package.
- Preserve Bambu filament/project arrays, including custom preset placeholders.
- Optionally deduplicate matching filament profiles with `--dedupe-filaments`.

## Install

Install Rust with `rustup` if needed:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Build the release binary:

```bash
cargo build --release
```

The binary will be created at:

```bash
target/release/three-mf-merger
```

On Windows, the binary is:

```text
target\release\three-mf-merger.exe
```

## Usage

```bash
three-mf-merger merge --output merged.3mf first.3mf second.3mf
```

From a local checkout:

```bash
./target/release/three-mf-merger merge \
  --output merged.3mf \
  first.3mf second.3mf
```

Overwrite an existing output file:

```bash
./target/release/three-mf-merger merge \
  --output merged.3mf \
  first.3mf second.3mf \
  --force
```

Merge three or more projects:

```bash
./target/release/three-mf-merger merge \
  --output merged.3mf \
  Mario.3mf Pikachu.3mf Star.3mf \
  --force
```

Deduplicate identical filament profiles:

```bash
./target/release/three-mf-merger merge \
  --output merged.3mf \
  Yoshi.3mf Yoshi.3mf \
  --force \
  --dedupe-filaments
```

`--dedupe-filaments` is intentionally conservative. Filaments are reused only
when material and process-relevant fields match, not merely when colors match.

## CLI Reference

```text
Usage: three-mf-merger merge [OPTIONS] --output <OUTPUT> <INPUTS>...

Arguments:
  <INPUTS>...              Input .3mf files

Options:
  -o, --output <OUTPUT>    Output .3mf file
      --force              Overwrite output if it already exists
      --printer-preset     Print merged printer preset settings to terminal
      --color-presets      Print merged filament colour presets to terminal
      --merge-filament     Merge filament settings from all inputs
      --merge-printer      Merge printer settings from all inputs
      --dedupe-filaments   Reuse existing merged filaments when profiles match
  -h, --help               Print help
```

## Notes On Presets

By default, the merger keeps the first input's printer/process context and
combines the filament-scaled data needed for the merged project to load.

`--merge-filament` and `--merge-printer` are available for experimentation, but
merged printer and filament presets should be reviewed carefully in Bambu Studio
before use.

## Development

Run tests:

```bash
cargo test
```

Format code:

```bash
cargo fmt
```

Build release:

```bash
cargo build --release
```

## Cross-Platform Builds

Cargo can build for multiple platforms, but the most reliable release setup is
to build each target on its native runner:

- Linux: `x86_64-unknown-linux-gnu`
- Windows: `x86_64-pc-windows-msvc`
- macOS Apple Silicon: `aarch64-apple-darwin`
- macOS Intel: `x86_64-apple-darwin`

For GitHub releases, use a CI matrix with `ubuntu-latest`, `windows-latest`, and
`macos-latest`/`macos-14`.

## Limitations

- The project is currently tuned for Bambu Studio `.3mf` packages.
- It is not a general-purpose 3MF repair tool.
- It does not validate printer safety. Bambu Studio may warn about custom
  presets; review those presets before printing.
- Strict 3MF validators may report slicer/vendor extension issues that Bambu
  Studio itself accepts.

## License

No license has been added yet.
