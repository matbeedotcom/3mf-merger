# 3MF And 3D Printing Customization Checklist

Use this checklist when validating whether a 3MF merge preserves all meaningful print/project customizations.

## Package Structure

- [ ] `[Content_Types].xml`
- [ ] Root package relationships: `_rels/.rels`
- [ ] Model relationships: `3D/_rels/3dmodel.model.rels`
- [ ] Main model part: `3D/3dmodel.model`
- [ ] Referenced object model parts: `3D/Objects/*.model`
- [ ] Referenced auxiliary files
- [ ] Referenced thumbnails
- [ ] Referenced metadata files
- [ ] Referenced textures
- [ ] Referenced vendor extension files
- [ ] Unique package paths after merge
- [ ] Unique relationship ids after merge
- [ ] Valid content types for every file extension
- [ ] Deterministic ZIP entry ordering

## Geometry And Object Data

- [ ] Mesh vertices
- [ ] Mesh triangles
- [ ] Component objects
- [ ] Component transforms
- [ ] Build items
- [ ] Build item transforms
- [ ] Printable flags
- [ ] Object ids
- [ ] Object UUIDs
- [ ] Object names
- [ ] Object type metadata
- [ ] Part ids
- [ ] Part names
- [ ] Source object ids
- [ ] Source volume ids
- [ ] Source offsets
- [ ] Mesh statistics
- [ ] Cut/split object data
- [ ] Connector/cut metadata

## Appearance And Painting

- [ ] Base materials
- [ ] Material groups
- [ ] Composite materials
- [ ] Multi-property groups
- [ ] Color groups
- [ ] Per-face color assignments
- [ ] Per-face material assignments
- [ ] Triangle `pid` references
- [ ] Triangle `p1`, `p2`, `p3` property indices
- [ ] Texture resources
- [ ] Texture coordinate groups
- [ ] Texture image files
- [ ] Texture relationship targets
- [ ] Painted face metadata
- [ ] Vendor-specific painting data

## Filament And Material Settings

- [ ] Filament ids
- [ ] Filament names
- [ ] Filament colors
- [ ] Filament types, such as PLA, PETG, TPU, ABS, ASA, support material
- [ ] Filament vendor/profile ids
- [ ] Filament density
- [ ] Filament diameter
- [ ] Filament cost
- [ ] Nozzle temperature
- [ ] Initial layer nozzle temperature
- [ ] Bed temperature
- [ ] Initial layer bed temperature
- [ ] Chamber temperature
- [ ] Max volumetric speed
- [ ] Flow ratio
- [ ] Pressure advance/K factor
- [ ] Retraction length
- [ ] Retraction speed
- [ ] Deretraction speed
- [ ] Z-hop
- [ ] Wipe settings
- [ ] Filament cooling settings
- [ ] Fan speeds
- [ ] Minimum layer time
- [ ] Softening or glass transition metadata
- [ ] Filament sequence/order
- [ ] AMS/MMU slot assignments
- [ ] Tool/extruder assignments
- [ ] Purge/flush volumes
- [ ] Flushing matrix

## Printer And Machine Settings

- [ ] Printer model
- [ ] Printer profile name/id
- [ ] Build volume dimensions
- [ ] Printable area
- [ ] Excluded bed areas
- [ ] Bed type
- [ ] Nozzle diameter
- [ ] Number of extruders/tools
- [ ] AMS/MMU availability
- [ ] Firmware flavor
- [ ] G-code dialect
- [ ] Start G-code
- [ ] End G-code
- [ ] Tool-change G-code
- [ ] Layer-change G-code
- [ ] Pause/change filament G-code
- [ ] Machine limits
- [ ] Max feedrates
- [ ] Max accelerations
- [ ] Jerk/junction deviation
- [ ] Travel limits
- [ ] Bed leveling settings
- [ ] Z offset
- [ ] Timelapse settings
- [ ] Air filtration settings
- [ ] Chamber fan settings
- [ ] Auxiliary fan settings

## Slicing Quality Settings

- [ ] Layer height
- [ ] Initial layer height
- [ ] Adaptive layer heights
- [ ] Variable layer height profile
- [ ] Line width
- [ ] Initial layer line width
- [ ] Wall line width
- [ ] Top/bottom line width
- [ ] Infill line width
- [ ] Support line width
- [ ] Wall count
- [ ] Top shell layers
- [ ] Bottom shell layers
- [ ] Top shell thickness
- [ ] Bottom shell thickness
- [ ] Infill density
- [ ] Infill pattern
- [ ] Sparse infill settings
- [ ] Internal solid infill settings
- [ ] Top surface pattern
- [ ] Bottom surface pattern
- [ ] Seam position
- [ ] Scarf seam settings
- [ ] Spiral/vase mode
- [ ] Fuzzy skin
- [ ] Arachne/classic wall generator
- [ ] Thin wall handling
- [ ] Gap fill
- [ ] Small feature handling
- [ ] Overhang wall handling
- [ ] Bridge flow
- [ ] Bridge speed
- [ ] Bridge angle
- [ ] Elephant foot compensation
- [ ] XY compensation
- [ ] Hole compensation
- [ ] Arc fitting
- [ ] Resolution/deviation tolerance

## Speed And Acceleration

- [ ] Outer wall speed
- [ ] Inner wall speed
- [ ] Infill speed
- [ ] Solid infill speed
- [ ] Top surface speed
- [ ] Initial layer speed
- [ ] Travel speed
- [ ] Support speed
- [ ] Bridge speed
- [ ] Gap fill speed
- [ ] Small perimeter speed
- [ ] Overhang speed
- [ ] Acceleration settings
- [ ] Initial layer acceleration
- [ ] Wall acceleration
- [ ] Infill acceleration
- [ ] Travel acceleration
- [ ] Bridge acceleration
- [ ] Accel-to-decel settings
- [ ] Jerk or square corner velocity

## Supports

- [ ] Support enabled/disabled
- [ ] Support type
- [ ] Support style
- [ ] Tree supports
- [ ] Normal supports
- [ ] Support placement
- [ ] Support angle threshold
- [ ] Support density
- [ ] Support pattern
- [ ] Support wall count
- [ ] Support interface enabled
- [ ] Support interface pattern
- [ ] Support interface density
- [ ] Top Z distance
- [ ] Bottom Z distance
- [ ] XY support distance
- [ ] Support/object gap
- [ ] Support brim
- [ ] Support painting/blockers/enforcers
- [ ] Raft settings

## Adhesion And First Layer

- [ ] Brim enabled
- [ ] Brim type
- [ ] Brim width
- [ ] Brim object gap
- [ ] Skirt loops
- [ ] Skirt distance
- [ ] Raft layers
- [ ] First layer speed
- [ ] First layer flow
- [ ] First layer temperature
- [ ] First layer bed temperature
- [ ] First layer fan behavior
- [ ] First layer acceleration
- [ ] Bed texture/profile

## Per-Object And Per-Part Overrides

- [ ] Object extruder assignment
- [ ] Part extruder assignment
- [ ] Object filament assignment
- [ ] Part filament assignment
- [ ] Object layer height
- [ ] Object wall count
- [ ] Object infill density
- [ ] Object infill pattern
- [ ] Object support settings
- [ ] Object seam settings
- [ ] Object speed settings
- [ ] Object acceleration settings
- [ ] Object flow settings
- [ ] Object brim/support brim settings
- [ ] Object fuzzy skin settings
- [ ] Object modifier meshes
- [ ] Negative volumes
- [ ] Support blockers
- [ ] Support enforcers
- [ ] Color painting per object/part/face

## Plates And Layout

- [ ] Plate count
- [ ] Plate ids/indices
- [ ] Plate names
- [ ] Plate preview images
- [ ] Plate no-light preview images
- [ ] Plate top images
- [ ] Plate pick images
- [ ] Plate JSON metadata
- [ ] Plate bounding boxes
- [ ] Plate object membership
- [ ] Plate object ids
- [ ] Plate object names
- [ ] Plate object positions
- [ ] Plate object rotations
- [ ] Plate object scales
- [ ] Plate bed type
- [ ] Plate first extruder
- [ ] Plate first layer time
- [ ] Plate sequence-print setting
- [ ] Plate nozzle diameter
- [ ] Plate filament colors
- [ ] Plate filament ids
- [ ] Multi-plate print order
- [ ] Auto-arrange state
- [ ] Object collision/overlap status

## Project Metadata

- [ ] Title
- [ ] Description
- [ ] Application name
- [ ] Application version
- [ ] Creation date
- [ ] Modification date
- [ ] Designer name
- [ ] Designer user id
- [ ] Designer cover
- [ ] License
- [ ] Origin
- [ ] Region
- [ ] Profile title
- [ ] Profile description
- [ ] Profile cover
- [ ] Profile id
- [ ] Profile user id
- [ ] Profile user name
- [ ] Design id
- [ ] Design model id
- [ ] Copyright metadata
- [ ] Project thumbnails
- [ ] Model pictures
- [ ] Profile pictures
- [ ] Assembly guide files
- [ ] Readme/instruction files

## Bambu Studio / Vendor Metadata

- [ ] `Metadata/model_settings.config`
- [ ] `Metadata/project_settings.config`
- [ ] `Metadata/slice_info.config`
- [ ] `Metadata/cut_information.xml`
- [ ] `Metadata/filament_sequence.json`
- [ ] `Metadata/filament_settings_*.config`
- [ ] `Metadata/layer_heights_profile.txt`
- [ ] `Metadata/plate_*.json`
- [ ] `Metadata/plate_*.png`
- [ ] `Metadata/top_*.png`
- [ ] `Metadata/pick_*.png`
- [ ] `Metadata/plate_no_light_*.png`
- [ ] Bambu package namespace declarations
- [ ] Bambu root thumbnail relationships
- [ ] Bambu cover-thumbnail relationships
- [ ] Per-object Bambu metadata
- [ ] Per-part Bambu metadata
- [ ] Plate preview promotion after merge
- [ ] Vendor project settings copied or merged
- [ ] Vendor object ids rewritten after merge
- [ ] Vendor plate references rewritten after merge

## Validation

- [ ] ZIP archive integrity passes
- [ ] All relationship targets exist
- [ ] All component `p:path` targets exist
- [ ] All content type defaults/overrides are present
- [ ] All object ids are unique where required
- [ ] All material/resource ids are unique where required
- [ ] All `objectid` references resolve
- [ ] All `pid` references resolve
- [ ] All texture references resolve
- [ ] All plate preview files resolve
- [ ] All metadata file references resolve
- [ ] Source package hashes remain unchanged
- [ ] Repeated merge is deterministic
- [ ] Output opens in target slicer
- [ ] Output opens in a neutral 3MF viewer
- [ ] Slicer-visible object count matches expectation
- [ ] Slicer-visible plate count matches expectation
- [ ] Slicer-visible colors/materials match expectation
- [ ] Slicer-visible filament assignments match expectation
- [ ] Slicer-visible support settings match expectation
- [ ] Slicer-visible per-object overrides match expectation
- [ ] Generated G-code preview matches expectation after slicing
