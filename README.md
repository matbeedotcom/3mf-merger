# 3MF Merger

`3mf-merger` combines multiple Bambu Studio `.3mf` project files into one merged `.3mf` package. It preserves models, plate metadata, thumbnails, object settings, filament mappings, and Bambu-specific configuration so merged projects load back into Bambu Studio.

Repository: <https://github.com/matbeedotcom/3mf-merger/>

## Browser App

The WASM app can be hosted on GitHub Pages. After Pages is enabled for GitHub
Actions deployments, pushes to `main` publish:

<https://matbeedotcom.github.io/3mf-merger/>

The root page loads the browser WASM package from `pkg/`.

## Install

```bash
cargo build --release
# binary: target/release/three-mf-merger (target\release\three-mf-merger.exe on Windows)
```

## Usage

```bash
three-mf-merger merge --output merged.3mf first.3mf second.3mf
```

Overwrite an existing output file:

```bash
three-mf-merger merge --output merged.3mf first.3mf second.3mf --force
```

Deduplicate identical filament profiles:

```bash
three-mf-merger merge --output merged.3mf Yoshi.3mf Yoshi.3mf --force --dedupe-filaments
```

## CLI Options

```
-o, --output <OUTPUT>      Output .3mf file (required)
      --force              Overwrite output if it already exists
      --dedupe-filaments   Reuse existing merged filaments when profiles match
```

## Development

```bash
cargo test
cargo fmt
cargo build --release
```

## License

No license has been added yet.
