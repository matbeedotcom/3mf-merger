# Change: Create 3MF Merger Project

## Why
The project needs a defined first capability: merge multiple `.3mf` files into one `.3mf` while keeping the per-model data that makes the files printable and visually correct. The repository already contains `Luigi.3mf` and `Yoshi.3mf`, which should serve as initial fixtures for validating the behavior.

## What Changes
- Define the expected behavior for accepting two or more `.3mf` inputs and producing one merged `.3mf`.
- Require preservation of geometry, painting, colors, filament/material assignments, metadata, build transforms, package relationships, and extension data where possible.
- Require deterministic id and relationship remapping to prevent collisions between source packages.
- Require validation and clear failure reporting for unsupported or ambiguous merge cases.

## Impact
- Establishes the baseline product specification before implementation.
- Guides future CLI/API design and test cases.
- Creates acceptance criteria using `Luigi.3mf` and `Yoshi.3mf` as initial sample inputs.
