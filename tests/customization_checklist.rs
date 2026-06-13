use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use tempfile::tempdir;
use zip::ZipArchive;

const LUIGI: &str = "Luigi.3mf";
const YOSHI: &str = "Yoshi.3mf";

#[test]
fn verifies_3mf_print_customization_checklist_for_luigi_yoshi_merge() {
    if !Path::new(LUIGI).exists() || !Path::new(YOSHI).exists() {
        return;
    }

    let merged = merge_fixture();
    let luigi = PackageSnapshot::read(LUIGI);
    let yoshi = PackageSnapshot::read(YOSHI);
    let output = PackageSnapshot::read(&merged);

    verify_package_structure(&luigi, &yoshi, &output);
    verify_geometry_and_object_data(&luigi, &yoshi, &output);
    verify_appearance_and_painting(&luigi, &yoshi, &output);
    verify_filament_material_printer_and_slicing_files(&luigi, &yoshi, &output);
    verify_per_object_overrides(&luigi, &yoshi, &output);
    verify_plates_and_layout(&luigi, &yoshi, &output);
    verify_project_and_vendor_metadata(&luigi, &yoshi, &output);
    verify_validation_invariants(&luigi, &yoshi, &output);
}

fn merge_fixture() -> PathBuf {
    let dir = tempdir().unwrap().keep();
    let output = dir.join("merged.3mf");
    three_mf_merger::merge_files(
        &[PathBuf::from(LUIGI), PathBuf::from(YOSHI)],
        &output,
        false, // force
        false, // printer_preset
        false, // color_presets
        false, // keep_first_printer
        false, // keep_first_filament
        false, // merge_filament
        false, // merge_printer
    )
    .unwrap();
    output
}

fn verify_package_structure(
    luigi: &PackageSnapshot,
    yoshi: &PackageSnapshot,
    output: &PackageSnapshot,
) {
    for required in [
        "[Content_Types].xml",
        "_rels/.rels",
        "3D/3dmodel.model",
        "3D/_rels/3dmodel.model.rels",
    ] {
        assert!(output.names.contains(required), "missing {required}");
    }

    assert_eq!(output.relationship_missing_targets(), Vec::<String>::new());
    assert_eq!(
        output.missing_content_type_extensions(),
        Vec::<String>::new()
    );

    let luigi_objects = luigi.object_part_names();
    let yoshi_objects = yoshi.object_part_names();
    let output_objects = output.object_part_names();
    assert_eq!(
        output_objects.len(),
        luigi_objects.len() + yoshi_objects.len()
    );

    for object in luigi_objects {
        assert!(
            output.names.contains(&object),
            "missing Luigi object {object}"
        );
    }
    for object in yoshi_objects {
        let file_name = object.rsplit('/').next().unwrap();
        let promoted = format!("3D/Objects/input-002-{file_name}");
        assert!(
            output.names.contains(&promoted),
            "missing promoted Yoshi object {promoted}"
        );
    }

    assert!(output
        .names
        .contains("Auxiliaries/.thumbnails/thumbnail_3mf.png"));
    assert!(output
        .names
        .contains("MergedInputs/input-002/Auxiliaries/.thumbnails/thumbnail_3mf.png"));
    assert!(output
        .names
        .contains("MergedInputs/input-002/Auxiliaries/Assembly Guide/Instruction.pdf"));
}

fn verify_geometry_and_object_data(
    luigi: &PackageSnapshot,
    yoshi: &PackageSnapshot,
    output: &PackageSnapshot,
) {
    assert_eq!(
        output.build_item_count(),
        luigi.build_item_count() + yoshi.build_item_count()
    );
    assert_eq!(
        output.model_object_count(),
        luigi.model_object_count() + yoshi.model_object_count()
    );
    assert_eq!(
        output.component_path_missing_targets(),
        Vec::<String>::new()
    );

    let object_ids = output.all_object_ids();
    assert_eq!(
        object_ids.len(),
        object_ids.iter().collect::<BTreeSet<_>>().len()
    );

    for object_id in output.model_objectid_refs() {
        assert!(
            object_ids.contains(&object_id),
            "objectid reference does not resolve: {object_id}"
        );
    }

    assert!(output
        .text("Metadata/model_settings.config")
        .contains("hat pin"));
    assert!(output
        .text("Metadata/model_settings.config")
        .contains("Yoshi插销-鼻子.stl"));
    assert_eq!(
        output.model_settings_object_count(),
        output.model_object_count()
    );
}

fn verify_appearance_and_painting(
    luigi: &PackageSnapshot,
    yoshi: &PackageSnapshot,
    output: &PackageSnapshot,
) {
    for tag in [
        "basematerials",
        "colorgroup",
        "texture2d",
        "texture2dgroup",
        "compositematerials",
        "multiproperties",
    ] {
        let input_count = luigi.resource_tag_count(tag) + yoshi.resource_tag_count(tag);
        assert!(
            output.resource_tag_count(tag) >= input_count,
            "resource tag {tag} was not preserved"
        );
    }

    let resource_ids = output.known_resource_ids();
    for pid in output.pid_refs() {
        assert!(
            resource_ids.contains(&pid),
            "pid reference does not resolve: {pid}"
        );
    }

    for ext in ["png", "webp"] {
        let input_assets = luigi.asset_count(ext) + yoshi.asset_count(ext);
        assert!(
            output.asset_count(ext) >= input_assets,
            "{ext} assets were not preserved"
        );
    }
}

fn verify_filament_material_printer_and_slicing_files(
    _luigi: &PackageSnapshot,
    _yoshi: &PackageSnapshot,
    output: &PackageSnapshot,
) {
    for direct in [
        "Metadata/project_settings.config",
        "Metadata/slice_info.config",
        "Metadata/filament_sequence.json",
        "Metadata/layer_heights_profile.txt",
        "Metadata/cut_information.xml",
        "Metadata/model_settings.config",
        "Metadata/filament_settings_11.config",
    ] {
        assert!(
            output.names.contains(direct),
            "missing direct metadata {direct}"
        );
    }

    for sidecar in [
        "MergedInputs/input-002/Metadata/slice_info.config",
        "MergedInputs/input-002/Metadata/layer_heights_profile.txt",
        "MergedInputs/input-002/Metadata/cut_information.xml",
    ] {
        assert!(
            output.names.contains(sidecar),
            "missing sidecar metadata {sidecar}"
        );
    }

    let project_settings = output.text("Metadata/project_settings.config");
    assert!(project_settings.contains("nozzle"));
    assert!(project_settings.contains("filament"));
    assert!(project_settings.contains("bed"));
    assert!(project_settings.contains("speed"));
    assert!(project_settings.contains("accel"));
    assert!(project_settings.contains("support"));

    let parsed_settings: serde_json::Value = serde_json::from_str(project_settings).unwrap();
    let colours = parsed_settings
        .get("filament_colour")
        .unwrap()
        .as_array()
        .unwrap();
    assert_eq!(colours.len(), 16);
}

fn verify_per_object_overrides(
    luigi: &PackageSnapshot,
    yoshi: &PackageSnapshot,
    output: &PackageSnapshot,
) {
    let settings = output.text("Metadata/model_settings.config");
    for needle in [
        "extruder",
        "support_style",
        "support_type",
        "seam_position",
        "mesh_stat",
        "source_object_id",
        "source_volume_id",
        "source_offset_x",
        "source_offset_y",
        "source_offset_z",
    ] {
        if luigi.contains_anywhere(needle) || yoshi.contains_anywhere(needle) {
            assert!(
                settings.contains(needle),
                "missing per-object override {needle}"
            );
        }
    }
}

fn verify_plates_and_layout(
    luigi: &PackageSnapshot,
    yoshi: &PackageSnapshot,
    output: &PackageSnapshot,
) {
    assert_eq!(
        output.plate_png_count(),
        luigi.plate_png_count() + yoshi.plate_png_count()
    );
    assert_eq!(
        output.top_png_count(),
        luigi.top_png_count() + yoshi.top_png_count()
    );
    assert_eq!(
        output.pick_png_count(),
        luigi.pick_png_count() + yoshi.pick_png_count()
    );
    assert_eq!(
        output.plate_no_light_count(),
        luigi.plate_no_light_count() + yoshi.plate_no_light_count()
    );
    assert_eq!(
        output.plate_json_count(),
        luigi.plate_json_count() + yoshi.plate_json_count()
    );

    for expected in [
        "Metadata/plate_8.png",
        "Metadata/plate_9.json",
        "Metadata/top_13.png",
        "Metadata/pick_13.png",
        "Metadata/plate_no_light_13.png",
    ] {
        assert!(
            output.names.contains(expected),
            "missing promoted plate file {expected}"
        );
    }

    let model = output.text("3D/3dmodel.model");
    assert!(model.contains(
        r#"<metadata name="Input002.Thumbnail_Middle">/Metadata/plate_8.png</metadata>"#
    ));

    let settings = output.text("Metadata/model_settings.config");
    for id in 1..=13 {
        assert!(
            settings.contains(&format!("key=\"plater_id\" value=\"{}\"", id))
                || settings.contains(&format!("key=\"plater_id\" value=\"{}\"", id))
        );
    }
    assert!(settings.contains("<assemble>"));
    assert!(settings.contains("</assemble>"));
    assert!(settings.contains("<assemble_item"));

    let sequence = output.text("Metadata/filament_sequence.json");
    let parsed_seq: serde_json::Value = serde_json::from_str(sequence).unwrap();
    for id in 1..=13 {
        assert!(parsed_seq.get(&format!("plate_{}", id)).is_some());
    }
}

fn verify_project_and_vendor_metadata(
    _luigi: &PackageSnapshot,
    _yoshi: &PackageSnapshot,
    output: &PackageSnapshot,
) {
    let model = output.text("3D/3dmodel.model");
    for needle in [
        "路易吉Luigi",
        "耀西Yoshi",
        "USfbbf0960dc6b4f",
        "USd3af4a4d3f5cb8",
        "ProfileTitle",
        "ProfileDescription",
        "Designer",
        "License",
        "BambuStudio:3mfVersion",
    ] {
        assert!(model.contains(needle), "missing project metadata {needle}");
    }

    let root_rels = output.text("_rels/.rels");
    assert!(root_rels.contains("metadata/thumbnail"));
    assert!(root_rels.contains("cover-thumbnail-middle"));
    assert!(root_rels.contains("cover-thumbnail-small"));
}

fn verify_validation_invariants(
    luigi: &PackageSnapshot,
    yoshi: &PackageSnapshot,
    output: &PackageSnapshot,
) {
    assert_eq!(output.relationship_missing_targets(), Vec::<String>::new());
    assert_eq!(
        output.component_path_missing_targets(),
        Vec::<String>::new()
    );
    assert_eq!(
        output.missing_content_type_extensions(),
        Vec::<String>::new()
    );
    assert_eq!(
        output.duplicate_model_relationship_ids(),
        Vec::<String>::new()
    );
    assert_eq!(
        output.duplicate_root_relationship_ids(),
        Vec::<String>::new()
    );

    for entry in &luigi.names {
        assert!(
            accounted_first_input(entry, &output.names),
            "missing Luigi entry representation: {entry}"
        );
    }
    for entry in &yoshi.names {
        assert!(
            accounted_later_input(entry, 2, 7, &output.names),
            "missing Yoshi entry representation: {entry}"
        );
    }
}

struct PackageSnapshot {
    names: BTreeSet<String>,
    text: BTreeMap<String, String>,
}

impl PackageSnapshot {
    fn read(path: impl AsRef<Path>) -> Self {
        let file = File::open(path.as_ref()).unwrap();
        let mut archive = ZipArchive::new(file).unwrap();
        let mut names = BTreeSet::new();
        let mut text = BTreeMap::new();

        for index in 0..archive.len() {
            let mut file = archive.by_index(index).unwrap();
            if file.is_dir() {
                continue;
            }
            let name = file.name().to_string();
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes).unwrap();
            if is_text_entry(&name) {
                if let Ok(decoded) = String::from_utf8(bytes) {
                    text.insert(name.clone(), decoded);
                }
            }
            names.insert(name);
        }

        Self { names, text }
    }

    fn text(&self, name: &str) -> &str {
        self.text.get(name).map(String::as_str).unwrap_or("")
    }

    fn contains_anywhere(&self, needle: &str) -> bool {
        self.text.values().any(|value| value.contains(needle))
    }

    fn object_part_names(&self) -> BTreeSet<String> {
        self.names
            .iter()
            .filter(|name| name.starts_with("3D/Objects/") && name.ends_with(".model"))
            .cloned()
            .collect()
    }

    fn build_item_count(&self) -> usize {
        self.text("3D/3dmodel.model")
            .matches("<item objectid=")
            .count()
    }

    fn model_object_count(&self) -> usize {
        count_resource_ids(self.text("3D/3dmodel.model"), "object")
    }

    fn model_settings_object_count(&self) -> usize {
        count_resource_ids(self.text("Metadata/model_settings.config"), "object")
    }

    fn model_object_ids(&self) -> BTreeSet<u32> {
        resource_ids(self.text("3D/3dmodel.model"), "object")
    }

    fn all_object_ids(&self) -> BTreeSet<u32> {
        let mut ids = self.model_object_ids();
        for object in self.object_part_names() {
            ids.extend(resource_ids(self.text(&object), "object"));
        }
        ids
    }

    fn model_objectid_refs(&self) -> Vec<u32> {
        attr_u32_values(self.text("3D/3dmodel.model"), "objectid")
    }

    fn component_path_missing_targets(&self) -> Vec<String> {
        attr_values(self.text("3D/3dmodel.model"), "p:path")
            .into_iter()
            .map(|value| value.trim_start_matches('/').to_string())
            .filter(|value| !self.names.contains(value))
            .collect()
    }

    fn known_resource_ids(&self) -> BTreeSet<u32> {
        let mut ids = BTreeSet::new();
        for tag in [
            "object",
            "basematerials",
            "colorgroup",
            "texture2d",
            "texture2dgroup",
            "compositematerials",
            "multiproperties",
        ] {
            ids.extend(resource_ids(self.text("3D/3dmodel.model"), tag));
            for object in self.object_part_names() {
                ids.extend(resource_ids(self.text(&object), tag));
            }
        }
        ids
    }

    fn pid_refs(&self) -> Vec<u32> {
        let mut refs = attr_u32_values(self.text("3D/3dmodel.model"), "pid");
        for object in self.object_part_names() {
            refs.extend(attr_u32_values(self.text(&object), "pid"));
        }
        refs
    }

    fn resource_tag_count(&self, tag: &str) -> usize {
        let mut count = count_resource_ids(self.text("3D/3dmodel.model"), tag);
        for object in self.object_part_names() {
            count += count_resource_ids(self.text(&object), tag);
        }
        count
    }

    fn asset_count(&self, ext: &str) -> usize {
        self.names
            .iter()
            .filter(|name| name.ends_with(&format!(".{ext}")))
            .count()
    }

    fn plate_png_count(&self) -> usize {
        self.plate_entry_count(".png")
    }

    fn plate_json_count(&self) -> usize {
        self.plate_entry_count(".json")
    }

    fn top_png_count(&self) -> usize {
        self.numbered_metadata_count("top_", ".png")
    }

    fn pick_png_count(&self) -> usize {
        self.numbered_metadata_count("pick_", ".png")
    }

    fn plate_no_light_count(&self) -> usize {
        self.numbered_metadata_count("plate_no_light_", ".png")
    }

    fn plate_entry_count(&self, suffix: &str) -> usize {
        self.numbered_metadata_count("plate_", suffix)
    }

    fn numbered_metadata_count(&self, prefix: &str, suffix: &str) -> usize {
        self.names
            .iter()
            .filter(|name| {
                let Some(rest) = name.strip_prefix(&format!("Metadata/{prefix}")) else {
                    return false;
                };
                rest.chars().next().is_some_and(|ch| ch.is_ascii_digit()) && name.ends_with(suffix)
            })
            .count()
    }

    fn relationship_missing_targets(&self) -> Vec<String> {
        let mut missing = Vec::new();
        for rel in self.names.iter().filter(|name| name.ends_with(".rels")) {
            let rel_dir = rel
                .split_once("/_rels/")
                .map(|(dir, _)| format!("{dir}/"))
                .unwrap_or_default();
            for target in attr_values(self.text(rel), "Target") {
                let normalized = if target.starts_with('/') {
                    target.trim_start_matches('/').to_string()
                } else {
                    format!("{rel_dir}{target}")
                };
                if !self.names.contains(&normalized) {
                    missing.push(format!("{rel}:{target}"));
                }
            }
        }
        missing
    }

    fn duplicate_model_relationship_ids(&self) -> Vec<String> {
        duplicates(attr_values(self.text("3D/_rels/3dmodel.model.rels"), "Id"))
    }

    fn duplicate_root_relationship_ids(&self) -> Vec<String> {
        duplicates(attr_values(self.text("_rels/.rels"), "Id"))
    }

    fn missing_content_type_extensions(&self) -> Vec<String> {
        let content_types = self.text("[Content_Types].xml");
        let defaults: BTreeSet<_> = attr_values(content_types, "Extension")
            .into_iter()
            .collect();
        let mut missing = BTreeSet::new();
        for name in &self.names {
            if name == "[Content_Types].xml" {
                continue;
            }
            let Some((_, ext)) = name.rsplit_once('.') else {
                continue;
            };
            if !defaults.contains(ext) {
                missing.insert(ext.to_string());
            }
        }
        missing.into_iter().collect()
    }
}

fn is_text_entry(name: &str) -> bool {
    name.ends_with(".xml")
        || name.ends_with(".model")
        || name.ends_with(".rels")
        || name.ends_with(".config")
        || name.ends_with(".json")
        || name.ends_with(".txt")
        || name == "[Content_Types].xml"
}

fn resource_ids(xml: &str, tag: &str) -> BTreeSet<u32> {
    let mut ids = BTreeSet::new();
    for chunk in xml.split(&format!("<{tag}")).skip(1) {
        if let Some(id) = attr_value_from_chunk(chunk, "id").and_then(|value| value.parse().ok()) {
            ids.insert(id);
        }
    }
    ids
}

fn count_resource_ids(xml: &str, tag: &str) -> usize {
    resource_ids(xml, tag).len()
}

fn attr_u32_values(xml: &str, attr: &str) -> Vec<u32> {
    attr_values(xml, attr)
        .into_iter()
        .filter_map(|value| value.parse().ok())
        .collect()
}

fn attr_values(xml: &str, attr: &str) -> Vec<String> {
    let mut values = Vec::new();
    for chunk in xml.split(&format!("{attr}=")).skip(1) {
        let mut chars = chunk.chars();
        let Some(quote) = chars.next() else {
            continue;
        };
        if quote != '"' && quote != '\'' {
            continue;
        }
        let value: String = chars.take_while(|ch| *ch != quote).collect();
        values.push(value);
    }
    values
}

fn attr_value_from_chunk(chunk: &str, attr: &str) -> Option<String> {
    attr_values(chunk.split('>').next().unwrap_or(""), attr)
        .into_iter()
        .next()
}

fn duplicates(values: Vec<String>) -> Vec<String> {
    let mut counts = BTreeMap::<String, usize>::new();
    for value in values {
        *counts.entry(value).or_default() += 1;
    }
    counts
        .into_iter()
        .filter_map(|(value, count)| (count > 1).then_some(value))
        .collect()
}

fn accounted_first_input(entry: &str, output_entries: &BTreeSet<String>) -> bool {
    matches!(
        entry,
        "[Content_Types].xml" | "_rels/.rels" | "3D/3dmodel.model" | "3D/_rels/3dmodel.model.rels"
    ) || output_entries.contains(entry)
}

fn accounted_later_input(
    entry: &str,
    input_index: usize,
    plate_offset: usize,
    output_entries: &BTreeSet<String>,
) -> bool {
    if matches!(
        entry,
        "[Content_Types].xml" | "_rels/.rels" | "3D/3dmodel.model" | "3D/_rels/3dmodel.model.rels"
    ) {
        return true;
    }
    if entry == "Metadata/project_settings.config" || entry == "Metadata/filament_sequence.json" {
        return output_entries.contains(entry);
    }
    if let Some(promoted_fil) = promoted_filament_settings_path(entry, 10) {
        return output_entries.contains(&promoted_fil);
    }
    if let Some(file_name) = entry.strip_prefix("3D/Objects/") {
        return output_entries.contains(&format!("3D/Objects/input-{input_index:03}-{file_name}"));
    }
    if entry == "Metadata/model_settings.config" {
        return output_entries.contains(entry);
    }
    if let Some(promoted) = promoted_plate_path(entry, plate_offset) {
        return output_entries.contains(&promoted);
    }
    output_entries.contains(&format!("MergedInputs/input-{input_index:03}/{entry}"))
}

fn promoted_filament_settings_path(entry: &str, filament_offset: usize) -> Option<String> {
    let file_name = entry.strip_prefix("Metadata/")?;
    if let Some(rest) = file_name.strip_prefix("filament_settings_") {
        let digit_count = rest.chars().take_while(|ch| ch.is_ascii_digit()).count();
        if digit_count > 0 {
            let source_number: usize = rest[..digit_count].parse().ok()?;
            return Some(format!(
                "Metadata/filament_settings_{}{}",
                filament_offset + source_number,
                &rest[digit_count..]
            ));
        }
    }
    None
}

fn promoted_plate_path(entry: &str, plate_offset: usize) -> Option<String> {
    let file_name = entry.strip_prefix("Metadata/")?;
    for prefix in ["plate_no_light_", "plate_", "top_", "pick_"] {
        if let Some(rest) = file_name.strip_prefix(prefix) {
            let digit_count = rest.chars().take_while(|ch| ch.is_ascii_digit()).count();
            if digit_count == 0 {
                continue;
            }
            let source_number: usize = rest[..digit_count].parse().ok()?;
            return Some(format!(
                "Metadata/{prefix}{}{}",
                plate_offset + source_number,
                &rest[digit_count..]
            ));
        }
    }
    None
}
