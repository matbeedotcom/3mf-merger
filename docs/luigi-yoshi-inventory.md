# Luigi + Yoshi Merge Inventory

This document records the current package-level comparison between `Luigi.3mf`, `Yoshi.3mf`, and the generated `merged.3mf`.

## Source Summary

| Feature | Luigi.3mf | Yoshi.3mf | merged.3mf |
| --- | ---: | ---: | ---: |
| ZIP file entries | 107 | 86 | 188 |
| Build items | 51 | 36 | 87 |
| `Metadata/plate_*.png` preview files | 7 | 6 | 13 |
| `Metadata/plate_*.json` files | 7 | 1 | 8 |

The output build item count is additive: `51 + 36 = 87`.

Yoshi does not currently contain seven `plate_*.json` files. It contains six plate preview image sets and one JSON plate file, `Metadata/plate_2.json`.

## Preserved Luigi Features

- Top-level package metadata remains in `/3D/3dmodel.model`, including Luigi title, profile title, description, design/profile ids, thumbnail metadata, and MakerWorld metadata.
- All Luigi object model parts remain under their original `/3D/Objects/...` paths.
- Luigi build items remain in the merged build section.
- Luigi `Metadata/plate_1` through `Metadata/plate_7` image/json/top/pick/no-light files remain under `Metadata/`.
- Luigi `Metadata/model_settings.config` object settings are included in the combined output `Metadata/model_settings.config`.
- Luigi root thumbnail and Bambu cover-thumbnail relationships remain in `_rels/.rels`.

## Preserved Yoshi Features

- Yoshi top-level model metadata is appended to `/3D/3dmodel.model`, including title, profile title, description, design/profile ids, thumbnail metadata, and MakerWorld metadata.
- Yoshi object model parts are copied under deterministic non-conflicting paths such as `/3D/Objects/input-002-object_79.model`.
- Yoshi component `p:path` references and object ids are rewritten to the copied object paths and remapped ids.
- Yoshi build items are appended to the merged build section.
- Yoshi `Metadata/model_settings.config` object settings are rewritten to the merged ids and included in the combined output `Metadata/model_settings.config`.
- Yoshi root thumbnail and Bambu cover-thumbnail relationships are preserved and point to copied thumbnail assets under `/MergedInputs/input-002/Auxiliaries/...`.
- Yoshi auxiliary files that are not promoted into first-class `Metadata/` paths are preserved under `/MergedInputs/input-002/...`, including assembly guide, model pictures, profile pictures, filament settings, project settings, slice info, layer heights, and cut information.

## Plate Promotion

Luigi plate metadata remains at its original indices, `1` through `7`.

Yoshi plate-like files are promoted after Luigi's max plate index:

| Yoshi source | Output |
| --- | --- |
| `Metadata/plate_1.png` | `Metadata/plate_8.png` |
| `Metadata/top_1.png` | `Metadata/top_8.png` |
| `Metadata/pick_1.png` | `Metadata/pick_8.png` |
| `Metadata/plate_no_light_1.png` | `Metadata/plate_no_light_8.png` |
| `Metadata/plate_2.json` | `Metadata/plate_9.json` |
| `Metadata/plate_2.png` | `Metadata/plate_9.png` |
| `Metadata/top_2.png` | `Metadata/top_9.png` |
| `Metadata/pick_2.png` | `Metadata/pick_9.png` |
| `Metadata/plate_no_light_2.png` | `Metadata/plate_no_light_9.png` |
| `Metadata/plate_3.png` through `Metadata/plate_6.png` | `Metadata/plate_10.png` through `Metadata/plate_13.png` |
| `Metadata/top_3.png` through `Metadata/top_6.png` | `Metadata/top_10.png` through `Metadata/top_13.png` |
| `Metadata/pick_3.png` through `Metadata/pick_6.png` | `Metadata/pick_10.png` through `Metadata/pick_13.png` |
| `Metadata/plate_no_light_3.png` through `Metadata/plate_no_light_6.png` | `Metadata/plate_no_light_10.png` through `Metadata/plate_no_light_13.png` |

## Automated Checks

The fixture tests assert:

- All Luigi input entries are represented in the output directly or through regenerated package-level files.
- All Yoshi input entries are represented in the output through object path remapping, plate promotion, merged config output, regenerated package-level files, or `/MergedInputs/input-002/...` preservation.
- The output includes both Luigi and Yoshi top-level metadata.
- Later-input top-level metadata names are prefixed, for example `Input002.Title`, so standard 3MF metadata names remain unique.
- The output includes both Luigi and Yoshi model settings in one `Metadata/model_settings.config`.
- The output includes remapped Yoshi object paths and ids.
- Repeated synthetic merges are byte-deterministic.

## lib3mf Comparison

The test suite includes a directed comparison against the third-party `lib3mf` crate. This comparison separates the package-level Bambu data we preserve from the standard 3MF data that `lib3mf` exposes.

| Check | Package-level merged output | `lib3mf` standard-visible output |
| --- | ---: | ---: |
| Build items | 87 | 87 |
| Top-level objects | 87 | 87 |
| Object model parts | 87 | 87 resource objects |
| Plate preview PNG sets | 13 | 0 |
| Plate JSON files | 8 | 0 |
| `face_property` color-paint attributes | 22,731 | unsupported vendor attribute |
| `paint_supports` support-paint attributes | 616 | unsupported vendor attribute |
| Build item transform attributes | 87 | preserved in package; stripped only for parser projection |
| Standard material/color resources | 0 | 0 |

Raw `merged.3mf` is intentionally still Bambu-flavored. `lib3mf` rejects it because the fixtures use non-standard triangle attributes (`face_property`, `paint_supports`). After stripping those vendor attributes for diagnostics, `lib3mf` next rejects preserved mirror transforms with negative determinants. The automated `lib3mf` comparison therefore parses a standard-3MF projection with Bambu triangle attributes and build transforms stripped, while the package-level assertions verify those original values remain present in the actual output.

The merge also rewrites later-input Production Extension UUIDs deterministically. This avoids duplicate `p:UUID` values when two source projects use the same UUID sequence.

## Remaining Manual Validation

ZIP/XML validation passes, but slicer-visible behavior still needs a manual Bambu Studio check. In particular, Bambu Studio should be used to confirm how it interprets promoted plate previews and appended model metadata.
