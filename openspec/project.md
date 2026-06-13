# Project: 3MF Merger

## Purpose
Build a tool that merges two or more 3MF files into a single valid 3MF file while preserving each input model's print-relevant data.

The initial validation fixtures are `Luigi.3mf` and `Yoshi.3mf` in the repository root.

## Goals
- Accept `N` input `.3mf` files where `N >= 2`.
- Produce exactly one merged `.3mf` output file.
- Preserve each source model's geometry, placement data, colors, painting, material/filament assignments, object metadata, build items, and other 3MF package relationships whenever possible.
- Avoid modifying the source `.3mf` files.
- Detect and report unsupported or conflicting package contents instead of silently dropping data.

## Non-Goals
- Repair invalid source 3MF files.
- Simplify meshes or optimize geometry.
- Convert between unrelated file formats.
- Re-slice models or infer slicer settings not present in the source packages.

## Constraints
- 3MF files are ZIP-based packages with XML parts and package relationships.
- The implementation must treat XML ids, relationship ids, object ids, material ids, texture ids, and build item references as scoped to their source file and remap them when merging.
- The merger must preserve vendor extension parts and relationships when they can be carried forward without ambiguity.

## Terminology
- **Input package**: One source `.3mf` file supplied to the merger.
- **Output package**: The single `.3mf` file produced by merging all input packages.
- **Resource**: A 3MF model resource such as objects, base materials, textures, color groups, components, or extension-specific resources.
- **Build item**: A model item in the 3MF build section that references an object resource and optional transform.
