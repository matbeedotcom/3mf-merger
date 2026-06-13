# Tasks

## Specification
- [x] Define project purpose and constraints in `openspec/project.md`.
- [x] Define the initial merge capability in `specs/merge-3mf/spec.md`.
- [x] Define implementation technology choices and architecture in `design.md`.

## Project Setup
- [x] Initialize a Rust CLI crate.
- [x] Add dependencies for CLI parsing, ZIP IO, XML rewriting, structured errors, temporary files, and tests.
- [ ] Add formatting, linting, and test commands to project documentation or CI once CI exists.

## Implementation
- [ ] Create a package parser that reads `.3mf` ZIP contents, `[Content_Types].xml`, root relationships, model relationships, and model XML parts.
- [ ] Implement namespace-aware model XML parsing for core 3MF resources, build items, metadata, and known reference-bearing attributes.
- [x] Create deterministic remapping tables for object ids, known material/resource ids, and package paths.
- [x] Merge model resources from all input packages into one output model in input order.
- [x] Merge build items while preserving per-item transforms and remapped object references.
- [x] Preserve or safely copy object model parts, textures, thumbnails, metadata, auxiliary files, and root auxiliary relationships.
- [x] Promote later-input Bambu plate preview assets into first-class `Metadata/plate_*`, `top_*`, `pick_*`, and `plate_no_light_*` entries.
- [x] Merge Bambu `Metadata/model_settings.config` object settings with remapped ids.
- [ ] Detect ambiguous unknown extension references and fail with a clear diagnostic instead of dropping data.
- [x] Write a valid output `.3mf` ZIP package through a temporary file and atomic rename.
- [x] Add the `3mf-merger merge --output merged.3mf input-a.3mf input-b.3mf ...` CLI entry point.
- [x] Add overwrite protection with an explicit `--force` option.

## Validation
- [x] Add fixture tests using `Luigi.3mf` and `Yoshi.3mf`.
- [x] Add fixture inventory tests proving each Luigi/Yoshi source entry is represented in the output.
- [x] Add synthetic fixture tests for colliding object ids, material/resource ids, and internal package paths.
- [x] Add deterministic output assertions for generated ids, paths, XML ordering, and ZIP entry ordering.
- [ ] Verify the output package opens in at least one common 3MF viewer or slicer.
- [x] Verify ZIP integrity for `Luigi.3mf` + `Yoshi.3mf` merged output.
- [x] Verify conflicting object paths from different inputs are remapped without reference breakage.
- [x] Add fixture coverage for relationship id collisions.
- [x] Verify object metadata and build item counts are preserved at the package/XML level.
- [x] Add third-party `lib3mf` verification for standard-visible object/build counts and documented Bambu-specific parser gaps.
- [ ] Verify colors, painting, material/filament assignments, object metadata, and build transforms are preserved in a slicer/viewer.
