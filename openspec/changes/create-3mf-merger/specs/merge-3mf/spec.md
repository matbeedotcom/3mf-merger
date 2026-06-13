# merge-3mf Specification

## ADDED Requirements

### Requirement: Accept Multiple 3MF Inputs
The system SHALL accept two or more `.3mf` input files and one output `.3mf` path.

#### Scenario: Merge Luigi and Yoshi
- **GIVEN** `Luigi.3mf` and `Yoshi.3mf` exist in the repository root
- **WHEN** the user requests a merge into `merged.3mf`
- **THEN** the system SHALL create one output package at `merged.3mf`
- **AND** the source files SHALL remain unchanged

#### Scenario: Reject Too Few Inputs
- **GIVEN** the user provides fewer than two `.3mf` input files
- **WHEN** the merge is requested
- **THEN** the system SHALL fail before writing an output package
- **AND** the error SHALL explain that at least two input files are required

#### Scenario: Refuse Accidental Overwrite
- **GIVEN** the requested output path already exists
- **WHEN** the user requests a merge without an explicit overwrite option
- **THEN** the system SHALL fail before writing an output package
- **AND** the error SHALL explain how to opt into overwriting

### Requirement: Preserve Model Geometry
The system SHALL preserve every mesh, component object, and build item from each input package in the output package.

#### Scenario: Preserve All Build Items
- **GIVEN** multiple input packages contain build items
- **WHEN** the packages are merged
- **THEN** every input build item SHALL have a corresponding output build item
- **AND** each output build item SHALL reference the remapped output object that represents its source object
- **AND** each output build item SHALL preserve its source transform

### Requirement: Preserve Visual And Material Settings
The system SHALL preserve print-relevant visual and material data, including colors, painting, textures, base materials, material groups, filament assignments, and slicer/vendor extension data when that data can be represented in the output package.

#### Scenario: Preserve Painted Models
- **GIVEN** an input model contains painted faces, color groups, texture coordinates, texture files, material references, or vendor extension data
- **WHEN** the package is merged
- **THEN** the output package SHALL contain equivalent visual/material resources
- **AND** all references from objects, triangles, components, and build items SHALL point to the remapped output resources

#### Scenario: Preserve Independent Filament Assignments
- **GIVEN** two input packages use overlapping material or filament ids for different meanings
- **WHEN** the packages are merged
- **THEN** the output package SHALL keep both assignments as distinct resources
- **AND** references from each source model SHALL resolve to the correct remapped resource

### Requirement: Remap Colliding Identifiers
The system SHALL treat ids and relationship ids from each input package as local to that package and SHALL remap them in the output package to avoid collisions.

#### Scenario: Object Id Collision
- **GIVEN** two input packages both define an object with id `1`
- **WHEN** the packages are merged
- **THEN** the output package SHALL assign unique object ids
- **AND** all build item, component, metadata, and resource references SHALL resolve to the corresponding remapped object

#### Scenario: Relationship Id Collision
- **GIVEN** two input packages contain relationships with the same relationship id
- **WHEN** the packages are merged
- **THEN** the output package SHALL assign unique relationship ids
- **AND** package parts SHALL reference the correct remapped relationship targets

### Requirement: Preserve Package Structure And Metadata
The system SHALL produce a valid 3MF ZIP package with required content types, relationships, model parts, metadata, and copied auxiliary parts.

#### Scenario: Copy Auxiliary Parts
- **GIVEN** an input package contains textures, thumbnails, metadata, or vendor extension parts
- **WHEN** the package is merged
- **THEN** the output package SHALL include those parts when they are referenced by preserved model data
- **AND** package paths SHALL be renamed when needed to avoid collisions
- **AND** relationship targets and content types SHALL be updated to the output paths

#### Scenario: Differing Auxiliary Part Path Collision
- **GIVEN** two input packages contain different bytes at the same internal package path
- **WHEN** the packages are merged
- **THEN** the later package's part SHALL be copied to a deterministic non-conflicting path
- **AND** all references to that part SHALL be rewritten to the deterministic output path

#### Scenario: Identical Auxiliary Part Path Collision
- **GIVEN** two input packages contain identical bytes at the same internal package path
- **WHEN** the packages are merged
- **THEN** the output package MAY store one shared copy of the part
- **AND** all references SHALL resolve to the shared output path

### Requirement: Report Unsupported Merge Cases
The system SHALL fail with a clear diagnostic when an input package contains data that cannot be safely merged without losing required settings.

#### Scenario: Unsupported Extension Data
- **GIVEN** an input package contains extension data whose references cannot be understood or safely copied
- **WHEN** preserving that data is required to keep painting, colors, filament assignments, or print settings intact
- **THEN** the system SHALL fail the merge
- **AND** the error SHALL identify the package and unsupported data category
- **AND** the output package SHALL not be written

### Requirement: Deterministic Output
For the same ordered list of input files, the system SHALL produce deterministic package paths, ids, relationships, and XML ordering.

#### Scenario: Repeat Same Merge
- **GIVEN** the same input files are merged in the same order twice
- **WHEN** both merges complete successfully
- **THEN** the logical package contents SHALL be equivalent
- **AND** generated ids and package paths SHALL be stable across runs

### Requirement: Write Output Atomically
The system SHALL avoid leaving a partial output package at the requested output path when a merge fails.

#### Scenario: Failure During Package Write
- **GIVEN** the merger has started creating an output package
- **WHEN** an error occurs before the package is complete
- **THEN** the requested output path SHALL not contain a partial package from the failed run
- **AND** the source input packages SHALL remain unchanged
