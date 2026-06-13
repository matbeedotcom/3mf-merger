# Design: 3MF Merger Implementation

## Technology Choices

### Language And Runtime
Use Rust for the first implementation.

Rationale:
- 3MF packages can contain large meshes and binary assets; Rust gives predictable memory use and fast ZIP/XML processing.
- The merger needs careful reference rewriting, where strong types reduce accidental id/path mixups.
- A single native CLI is easy to run locally and in CI without requiring a Python or Node runtime.

### Suggested Crates
- `clap`: CLI argument parsing.
- `zip`: reading and writing `.3mf` ZIP packages.
- `quick-xml`: streaming XML read/write for 3MF model parts and relationships.
- `indexmap`: deterministic insertion-ordered maps for stable output.
- `camino`: UTF-8 paths for internal package paths and user-facing paths.
- `thiserror` and `anyhow`: structured library errors and CLI error reporting.
- `tempfile`: atomic output staging and tests.
- `insta` or snapshot fixtures: deterministic XML/package manifest assertions.

### Project Shape
- `src/main.rs`: CLI entry point and exit-code handling.
- `src/lib.rs`: public merge API used by CLI and tests.
- `src/package.rs`: ZIP package read/write, content types, package paths, and relationship files.
- `src/model.rs`: typed representation of 3MF model XML, resources, build items, and extension namespaces.
- `src/remap.rs`: id/path/relationship remapping tables.
- `src/merge.rs`: orchestration for combining parsed packages into one output package.
- `tests/fixtures/`: fixture copies or fixture references for `Luigi.3mf` and `Yoshi.3mf`.

## CLI Contract

Initial command shape:

```bash
3mf-merger merge --output merged.3mf Luigi.3mf Yoshi.3mf
```

Behavior:
- Require at least two input files.
- Refuse to overwrite an existing output file unless `--force` is provided.
- Write to a temporary file in the output directory and rename it into place only after the package is complete.
- Print concise diagnostics with the input file and data category when a merge cannot be completed.

Optional later flags:
- `--dry-run`: validate mergeability without writing output.
- `--report report.json`: write a machine-readable summary of copied parts, remapped ids, and warnings.
- `--layout preserve|centered|sequence`: control placement policy if future collision avoidance is needed.

## Merge Strategy

### Package Loading
For each input package:
- Open the `.3mf` as a ZIP archive.
- Read `[Content_Types].xml`.
- Read root `_rels/.rels` to locate the primary 3D model part.
- Read model part relationships, auxiliary parts, and extension/vendor parts.
- Preserve unknown parts as binary payloads when their relationships can be safely carried forward.

### Model Merge
Use the first input package as the base for output-level defaults and package metadata, then append resources and build items from each input package in input order.

For every input package, create a scoped remap context:
- Object ids.
- Resource ids such as base materials, color groups, texture coordinates, composite materials, multi-property groups, and extension resources.
- Relationship ids.
- Package part paths.
- Content type overrides.

All id-producing operations must be deterministic. Recommended policy:
- Allocate new numeric ids monotonically in input order.
- Keep the first package's ids only when they do not conflict and doing so does not make the code path special-cased in a fragile way.
- Prefer deterministic renaming over content hashing for package part collisions, e.g. `/3D/Metadata/input-002/name.ext`.

### XML Handling
Use namespace-aware XML processing.

Rules:
- Preserve known 3MF core elements with typed parsing and serialization.
- Preserve unknown namespaced elements and attributes as raw XML nodes when they can be copied without unresolved references.
- Rewrite all known reference-bearing attributes through the remap context.
- Fail rather than drop unknown data that may affect color, painting, materials, filament, or slicer behavior.

### Auxiliary Parts
Copy referenced auxiliary parts, including textures, thumbnails, metadata, and vendor extension files.

When two packages contain the same internal path:
- If the bytes are identical, the output may share one copied part.
- If the bytes differ, assign a deterministic package path for the later input and rewrite relationships to the new path.

### Output Package
The writer must create:
- `[Content_Types].xml`.
- Root `_rels/.rels`.
- One merged model part, preferably `/3D/3dmodel.model`.
- Relationship files for the model part and any copied parts that require them.
- All referenced auxiliary binary/XML parts.

ZIP entry ordering and XML serialization should be deterministic for stable tests.

## Validation Strategy

Automated tests:
- Merge `Luigi.3mf` and `Yoshi.3mf` and assert that output package structure is valid.
- Assert source file hashes do not change.
- Assert all build items from both inputs exist in the output.
- Assert known material/color/texture references resolve after remapping.
- Add synthetic fixtures with colliding object ids, material ids, relationship ids, and internal paths.

Manual validation:
- Open the merged output in a common slicer/viewer.
- Visually confirm Luigi and Yoshi both appear with expected colors/painting/material assignments.
- Confirm slicer filament/material mappings remain distinct where source packages differ.

## Risks And Tradeoffs

### Vendor Extensions
3MF files produced by slicers often include vendor-specific extensions. Copying unknown extension data is safer than dropping it, but reference-bearing unknown XML can still be unsafe. The first implementation should preserve what it can prove is self-contained and fail clearly for ambiguous references.

### Placement
The initial merge preserves each input build item's transform exactly. If two models overlap because they were authored at the same origin, that is still a valid merge. Automatic layout can be added later as an explicit option.

### Full 3MF Coverage
The initial implementation should prioritize the core 3MF model, materials, colors, textures, build items, package relationships, and common slicer metadata. Less common extensions should be added behind tests as they are encountered.
