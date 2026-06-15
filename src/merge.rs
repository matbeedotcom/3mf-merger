use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use tempfile::NamedTempFile;
use thiserror::Error;

use crate::package::{write_package, write_package_to_bytes, Package};
use crate::rewrite::{
    collect_metadata_elements, collect_resource_ids, config_assemble_item_elements,
    config_object_elements, config_plate_elements, parse_relationships, prefix_metadata_name,
    rewrite_bambu_assemble_item_element, rewrite_bambu_model_settings, rewrite_bambu_plate_element,
    rewrite_metadata_path_text, rewrite_model_xml, rewrite_production_uuids, split_model,
    Relationship, Remap,
};

const CONTENT_TYPES: &str = "[Content_Types].xml";
const ROOT_RELS: &str = "_rels/.rels";
const MODEL: &str = "3D/3dmodel.model";
const MODEL_RELS: &str = "3D/_rels/3dmodel.model.rels";
const MODEL_SETTINGS: &str = "Metadata/model_settings.config";

#[derive(Debug, Error)]
pub enum MergeError {
    #[error("at least two input .3mf files are required")]
    TooFewInputs,
    #[error("output file already exists: {0}; pass --force to overwrite")]
    OutputExists(PathBuf),
}

#[derive(Debug, Clone, Default)]
pub struct MergeOptions {
    pub force: bool,
    pub printer_preset: bool,
    pub color_presets: bool,
    pub keep_first_printer: bool,
    pub keep_first_filament: bool,
    pub merge_filament: bool,
    pub merge_printer: bool,
    pub dedupe_filaments: bool,
}

pub fn merge_files_wasm(inputs: &[Vec<u8>], options: &MergeOptions) -> Result<Vec<u8>> {
    if inputs.len() < 2 {
        return Err(MergeError::TooFewInputs.into());
    }

    let loaded: Vec<Package> = inputs
        .iter()
        .map(|bytes| Package::from_bytes(bytes))
        .collect::<Result<Vec<_>>>()?;

    let entries = merge_packages_loaded(&loaded, options)?;
    write_package_to_bytes(&entries)
}

pub fn merge_files(
    inputs: &[PathBuf],
    output: &Path,
    force: bool,
    printer_preset: bool,
    color_presets: bool,
    _keep_first_printer: bool,
    _keep_first_filament: bool,
    merge_filament: bool,
    merge_printer: bool,
    dedupe_filaments: bool,
) -> Result<()> {
    if inputs.len() < 2 {
        return Err(MergeError::TooFewInputs.into());
    }
    if output.exists() && !force {
        return Err(MergeError::OutputExists(output.to_path_buf()).into());
    }

    let options = MergeOptions {
        force,
        printer_preset,
        color_presets,
        keep_first_printer: true,
        keep_first_filament: true,
        merge_filament,
        merge_printer,
        dedupe_filaments,
    };

    let loaded: Vec<Package> = inputs
        .iter()
        .map(|p| Package::read(p.as_ref()))
        .collect::<Result<Vec<_>>>()?;

    let merged = merge_packages_loaded(&loaded, &options)?;
    let output_dir = output.parent().unwrap_or_else(|| Path::new("."));
    let temp = NamedTempFile::new_in(output_dir).with_context(|| {
        format!(
            "failed to create temporary output in {}",
            output_dir.display()
        )
    })?;

    write_package(temp.path(), &merged)?;

    if force && output.exists() {
        fs::remove_file(output)
            .with_context(|| format!("failed to remove existing output {}", output.display()))?;
    }
    temp.persist(output)
        .map_err(|err| err.error)
        .with_context(|| format!("failed to move merged package to {}", output.display()))?;

    Ok(())
}

fn merge_packages_loaded(
    loaded: &[Package],
    options: &MergeOptions,
) -> Result<BTreeMap<String, Vec<u8>>> {
    let printer_preset = options.printer_preset;
    let color_presets = options.color_presets;
    let merge_filament = options.merge_filament;
    let merge_printer = options.merge_printer;
    let dedupe_filaments = options.dedupe_filaments;

    let mut filament_counts = Vec::with_capacity(loaded.len());
    let mut cumulative_filaments = Vec::with_capacity(loaded.len());
    let mut total_filaments = 0;

    for package in loaded {
        let count =
            if let Some(settings_bytes) = package.entries.get("Metadata/project_settings.config") {
                let json: serde_json::Value =
                    serde_json::from_slice(settings_bytes).unwrap_or(serde_json::Value::Null);
                get_filament_count(&json)
            } else {
                0
            };
        filament_counts.push(count);
        cumulative_filaments.push(total_filaments);
        total_filaments += count;
    }

    let mut plate_counts = Vec::with_capacity(loaded.len());
    let mut plate_offsets = Vec::with_capacity(loaded.len());
    let mut cumulative_plates = 0;
    for package in loaded {
        let plate_count = get_plate_count(package);
        plate_counts.push(plate_count);
        plate_offsets.push(cumulative_plates);
        cumulative_plates += plate_count;
    }

    let mut identify_id_offsets = Vec::with_capacity(loaded.len());
    let mut cumulative_identify_id_offset = 0;
    for package in loaded {
        identify_id_offsets.push(cumulative_identify_id_offset);
        cumulative_identify_id_offset += get_max_identify_id(package);
    }

    let mut next_id = 1;
    let mut merged_resources = String::new();
    let mut merged_build = String::new();
    let mut appended_metadata = String::new();
    let mut merged_model_settings = String::new();
    let mut merged_plates = String::new();
    let mut merged_assemble_items = String::new();
    let mut merged_project_settings: Option<Vec<u8>> = None;
    let mut merged_filament_sequence: Option<Vec<u8>> = None;
    let mut output_entries = BTreeMap::new();
    let mut output_object_paths = BTreeSet::new();
    let mut model_rel_targets = Vec::new();
    let mut root_relationships = Vec::new();
    let mut next_plate_index = 1;
    let mut first_sections = None;

    for (index, package) in loaded.iter().enumerate() {
        let source_model = package.get_text(MODEL)?;
        let mut source_ids = collect_resource_ids(&source_model)?;

        for path in package.entries.keys().filter(|name| is_object_model(name)) {
            let xml = package.get_text(path)?;
            source_ids.extend(collect_resource_ids(&xml)?);
        }

        source_ids.sort_unstable();
        source_ids.dedup();

        let mut remap = Remap::default();
        remap.filament_offset = cumulative_filaments[index];
        for source_id in source_ids {
            remap.ids.insert(source_id, next_id);
            next_id += 1;
        }

        let mut next_uuid_index = 1;
        for path in package.entries.keys().filter(|name| is_object_model(name)) {
            let mapped = allocate_object_path(index, path, &mut output_object_paths);
            remap.paths.insert(path.clone(), mapped);
        }

        for (source_path, mapped_path) in &remap.paths {
            let xml = package.get_text(source_path)?;
            let mut rewritten = rewrite_model_xml(&xml, &remap)
                .with_context(|| format!("failed to rewrite object model part {source_path}"))?;
            if index > 0 {
                rewritten = rewrite_production_uuids(&rewritten, index + 1, &mut next_uuid_index)
                    .with_context(|| {
                        format!("failed to rewrite production UUIDs for {source_path}")
                    })?;
            }
            output_entries.insert(mapped_path.clone(), rewritten.into_bytes());
            model_rel_targets.push(format!("/{mapped_path}"));
        }

        let object_to_plate = get_object_plate_map(package)?;
        let shifted_source_model = rewrite_build_item_transforms(
            &source_model,
            index,
            &plate_offsets,
            plate_counts[index],
            cumulative_plates,
            &object_to_plate,
        )?;

        let mut rewritten_model = rewrite_model_xml(&shifted_source_model, &remap)
            .with_context(|| format!("failed to rewrite top-level model for input #{index}"))?;
        if index > 0 {
            rewritten_model =
                rewrite_production_uuids(&rewritten_model, index + 1, &mut next_uuid_index)
                    .with_context(|| {
                        format!("failed to rewrite production UUIDs for input #{index}")
                    })?;
        }
        let sections = split_model(&rewritten_model)?;

        if first_sections.is_none() {
            first_sections = Some(sections.clone());
        }
        merged_resources.push_str(&sections.resources_inner);
        if !sections.resources_inner.ends_with('\n') {
            merged_resources.push('\n');
        }
        merged_build.push_str(&sections.build_inner);
        if !sections.build_inner.ends_with('\n') {
            merged_build.push('\n');
        }

        let p1 = if index == 0 { 0 } else { next_plate_index - 1 };
        let p2 = get_plate_count(package);

        let plate_offset = if index == 0 { 0 } else { next_plate_index - 1 };
        let auxiliary_paths = copy_auxiliary_entries(
            index,
            package,
            &mut output_entries,
            &mut next_plate_index,
            remap.filament_offset,
            &remap,
            &plate_offsets,
            plate_counts[index],
            cumulative_plates,
        )?;
        if index > 0 {
            for metadata in collect_metadata_elements(&rewritten_model)? {
                let metadata = rewrite_metadata_path_text(&metadata, &auxiliary_paths);
                let metadata = prefix_metadata_name(&metadata, &format!("Input{:03}.", index + 1))?;
                appended_metadata.push_str(" ");
                appended_metadata.push_str(&metadata);
                appended_metadata.push('\n');
            }
        }

        let n_before = cumulative_filaments[index];
        let n_after = total_filaments - cumulative_filaments[index] - filament_counts[index];

        if let Some(settings) = package.entries.get(MODEL_SETTINGS) {
            let settings = String::from_utf8(settings.clone())
                .context("model_settings.config is not UTF-8")?;
            let rewritten_settings = rewrite_bambu_model_settings(&settings, &remap)?;
            for object in config_object_elements(&rewritten_settings)? {
                merged_model_settings.push_str("  ");
                merged_model_settings.push_str(&object);
                merged_model_settings.push('\n');
            }
            for plate in config_plate_elements(&settings)? {
                let rewritten_plate = rewrite_bambu_plate_element(
                    &plate,
                    &remap,
                    plate_offset,
                    identify_id_offsets[index],
                    n_before,
                    n_after,
                )?;
                merged_plates.push_str("  ");
                merged_plates.push_str(&rewritten_plate);
                merged_plates.push('\n');
            }
            for assemble_item in config_assemble_item_elements(&settings)? {
                let rewritten_assemble_item =
                    rewrite_bambu_assemble_item_element(&assemble_item, &remap)?;
                merged_assemble_items.push_str("   ");
                merged_assemble_items.push_str(&rewritten_assemble_item);
                merged_assemble_items.push('\n');
            }
        }

        if let Some(settings_bytes) = package.entries.get("Metadata/project_settings.config") {
            if let Some(master_bytes) = &merged_project_settings {
                merged_project_settings = Some(merge_project_settings(
                    master_bytes,
                    settings_bytes,
                    p1,
                    p2,
                    merge_printer,
                    merge_filament,
                )?);
            } else {
                merged_project_settings = Some(settings_bytes.clone());
            }
        }
        if let Some(seq_bytes) = package.entries.get("Metadata/filament_sequence.json") {
            if let Some(master_bytes) = &merged_filament_sequence {
                merged_filament_sequence = Some(merge_filament_sequence(
                    master_bytes,
                    seq_bytes,
                    plate_offset,
                )?);
            } else {
                merged_filament_sequence = Some(seq_bytes.clone());
            }
        }

        root_relationships.extend(rewrite_root_relationships(package, &auxiliary_paths)?);
    }

    let first = first_sections.context("no model sections were loaded")?;
    let merged_model = format!(
        "{}{}<resources>{}</resources>\n {}{}\n</build>{}",
        first.pre_resources,
        appended_metadata,
        merged_resources,
        first.build_open,
        merged_build,
        first.post_build
    );

    output_entries.insert(CONTENT_TYPES.to_string(), content_types_xml().into_bytes());
    output_entries.insert(
        ROOT_RELS.to_string(),
        root_relationships_xml(&root_relationships).into_bytes(),
    );
    output_entries.insert(MODEL.to_string(), merged_model.into_bytes());
    output_entries.insert(
        MODEL_RELS.to_string(),
        model_relationships_xml(&model_rel_targets).into_bytes(),
    );
    if let Some(settings) = merged_project_settings {
        output_entries.insert("Metadata/project_settings.config".to_string(), settings);
    }
    if let Some(seq) = merged_filament_sequence {
        output_entries.insert("Metadata/filament_sequence.json".to_string(), seq);
    }
    output_entries.insert(
        MODEL_SETTINGS.to_string(),
        model_settings_xml(
            &merged_model_settings,
            &merged_plates,
            &merged_assemble_items,
        )
        .into_bytes(),
    );

    if dedupe_filaments {
        dedupe_merged_filaments(&mut output_entries)?;
    }

    // Debug output flags
    if printer_preset {
        if let Some(settings_bytes) = output_entries.get("Metadata/project_settings.config") {
            if let Ok(settings) = serde_json::from_slice::<serde_json::Value>(settings_bytes) {
                print_printer_preset(&settings);
            }
        }
    }
    if color_presets {
        if let Some(settings_bytes) = output_entries.get("Metadata/project_settings.config") {
            if let Ok(settings) = serde_json::from_slice::<serde_json::Value>(settings_bytes) {
                print_color_presets(&settings);
            }
        }
    }

    Ok(output_entries)
}

fn print_printer_preset(settings: &serde_json::Value) {
    println!("=== PRINTER PRESET ===");
    let keys = [
        "print_settings_id",
        "printer_settings_id",
        "print_compatible_printers",
        "compatible_printers",
        "compatible_printers_condition",
        "default_print_profile",
        "primary_printing_profile",
        "bed_type",
        "nozzle_diameter",
        "hotend_type",
        "machine_start_gcode",
        "machine_end_gcode",
        "bed_custom_model",
        "bed_custom_texture",
        "printable_area",
        "printable_height",
    ];
    for key in keys {
        if let Some(val) = settings.get(key) {
            let json_str = serde_json::to_string_pretty(val).unwrap_or_default();
            println!("{}: {}", key, json_str);
        }
    }
}

fn print_color_presets(settings: &serde_json::Value) {
    println!("=== FILAMENT COLOUR PRESETS ===");
    let keys = [
        "filament_colour",
        "filament_colour_type",
        "filament_extruder_variant",
        "filament_type",
        "filament_vendor",
        "filament_name",
        "filament_settings_id",
        "filament_flow_ratio",
        "default_filament_profile",
        "filament_shrink",
        "filament_soluble",
        "filament_retraction_length",
        "filament_retraction_speed",
        "filament_deretraction_speed",
        "filament_z_hop",
        "filament_wipe",
        "filament_wipe_distance",
    ];
    for key in keys {
        if let Some(val) = settings.get(key) {
            let json_str = serde_json::to_string_pretty(val).unwrap_or_default();
            println!("{}: {}", key, json_str);
        }
    }
}

fn is_object_model(path: &str) -> bool {
    path.starts_with("3D/Objects/") && path.ends_with(".model")
}

fn skip_direct_copy(path: &str) -> bool {
    path == CONTENT_TYPES
        || path == ROOT_RELS
        || path == MODEL
        || path == MODEL_RELS
        || path == MODEL_SETTINGS
        || path == "Metadata/project_settings.config"
        || path == "Metadata/filament_sequence.json"
        || is_object_model(path)
}

fn allocate_object_path(index: usize, source_path: &str, used: &mut BTreeSet<String>) -> String {
    if index == 0 && !used.contains(source_path) {
        used.insert(source_path.to_string());
        return source_path.to_string();
    }

    let file_name = source_path.rsplit('/').next().unwrap_or(source_path);
    let mut candidate = format!("3D/Objects/input-{:03}-{file_name}", index + 1);
    let mut suffix = 2;
    while used.contains(&candidate) {
        candidate = format!("3D/Objects/input-{:03}-{suffix}-{file_name}", index + 1);
        suffix += 1;
    }
    used.insert(candidate.clone());
    candidate
}

fn copy_auxiliary_entries(
    index: usize,
    package: &Package,
    output_entries: &mut BTreeMap<String, Vec<u8>>,
    next_plate_index: &mut usize,
    filament_offset: usize,
    remap: &Remap,
    plate_offsets: &[usize],
    source_plate_count: usize,
    total_plate_count: usize,
) -> Result<BTreeMap<String, String>> {
    let mut copied_paths = BTreeMap::new();
    let plate_plan = plan_plate_promotions(index, package, *next_plate_index)?;
    if index == 0 {
        *next_plate_index = max_plate_index(package).unwrap_or(0) + 1;
    } else {
        *next_plate_index += plate_plan
            .iter()
            .filter(|(source, _)| is_plate_json(source))
            .count()
            .max(max_plate_index(package).unwrap_or(0));
    }

    for (path, bytes) in &package.entries {
        if skip_direct_copy(path) {
            continue;
        }

        let target = if let Some(promoted) = plate_plan.get(path) {
            promoted.clone()
        } else if let Some(fil_num) = metadata_filament_settings_number(path) {
            if index == 0 {
                path.clone()
            } else {
                // Check if this filament settings file is identical to an existing one
                // If so, don't create a duplicate - remap to the existing one
                if let Ok(config_str) = String::from_utf8(bytes.clone()) {
                    if let Ok(config_json) = serde_json::from_str::<serde_json::Value>(&config_str)
                    {
                        let settings_id = config_json
                            .get("filament_settings_id")
                            .and_then(|v| v.as_array())
                            .and_then(|arr| arr.first())
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let name = config_json
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");

                        // Check if we already have this exact filament settings in output_entries
                        let mut found_match = false;
                        for (existing_path, existing_bytes) in output_entries.iter() {
                            if let Ok(existing_str) = String::from_utf8(existing_bytes.clone()) {
                                if let Ok(existing_json) =
                                    serde_json::from_str::<serde_json::Value>(&existing_str)
                                {
                                    let existing_id = existing_json
                                        .get("filament_settings_id")
                                        .and_then(|v| v.as_array())
                                        .and_then(|arr| arr.first())
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("");
                                    let existing_name = existing_json
                                        .get("name")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("");
                                    if existing_id == settings_id
                                        && existing_name == name
                                        && !settings_id.is_empty()
                                    {
                                        found_match = true;
                                        copied_paths.insert(path.clone(), existing_path.clone());
                                        break;
                                    }
                                }
                            }
                        }
                        if found_match {
                            continue; // Skip creating duplicate, mapped in previous iteration
                        }
                    }
                }
                format!(
                    "Metadata/filament_settings_{}.config",
                    fil_num + filament_offset
                )
            }
        } else if index == 0 && !output_entries.contains_key(path) {
            path.clone()
        } else if output_entries
            .get(path)
            .is_some_and(|existing| existing == bytes)
        {
            continue;
        } else {
            format!("MergedInputs/input-{:03}/{}", index + 1, path)
        };

        if output_entries.contains_key(&target) {
            bail!("internal output path collision at {target}");
        }

        let mut target_bytes = bytes.clone();
        if is_plate_json(path) {
            let source_plate = metadata_plate_number(path).unwrap_or(1);
            let (dx, dy) = get_plate_shift(
                index,
                source_plate,
                plate_offsets,
                source_plate_count,
                total_plate_count,
            );
            target_bytes = rewrite_plate_json(&target_bytes, remap, filament_offset, dx, dy)?;
        }

        copied_paths.insert(path.clone(), target.clone());
        output_entries.insert(target, target_bytes);
    }

    Ok(copied_paths)
}

fn plan_plate_promotions(
    index: usize,
    package: &Package,
    next_plate_index: usize,
) -> Result<BTreeMap<String, String>> {
    if index == 0 {
        return Ok(BTreeMap::new());
    }

    let mut plate_numbers: Vec<_> = package
        .entries
        .keys()
        .filter_map(|path| metadata_plate_number(path))
        .collect();
    plate_numbers.sort_unstable();
    plate_numbers.dedup();

    let mut plan = BTreeMap::new();
    for (offset, source_number) in plate_numbers.iter().enumerate() {
        let target_number = next_plate_index + offset;
        for path in package.entries.keys() {
            if metadata_plate_number(path) == Some(*source_number) {
                let target = rewrite_metadata_plate_number(path, target_number)
                    .with_context(|| format!("failed to promote plate metadata path {path}"))?;
                plan.insert(path.clone(), target);
            }
        }
    }

    Ok(plan)
}

fn max_plate_index(package: &Package) -> Option<usize> {
    package
        .entries
        .keys()
        .filter_map(|path| metadata_plate_number(path))
        .max()
}

fn metadata_plate_number(path: &str) -> Option<usize> {
    let file_name = path.strip_prefix("Metadata/")?;
    let prefixes = ["plate_no_light_", "plate_", "top_", "pick_"];
    for prefix in prefixes {
        if let Some(rest) = file_name.strip_prefix(prefix) {
            let digits: String = rest.chars().take_while(|ch| ch.is_ascii_digit()).collect();
            if !digits.is_empty() {
                return digits.parse().ok();
            }
        }
    }
    None
}

fn rewrite_metadata_plate_number(path: &str, target_number: usize) -> Option<String> {
    let file_name = path.strip_prefix("Metadata/")?;
    let prefixes = ["plate_no_light_", "plate_", "top_", "pick_"];
    for prefix in prefixes {
        if let Some(rest) = file_name.strip_prefix(prefix) {
            let digit_count = rest.chars().take_while(|ch| ch.is_ascii_digit()).count();
            if digit_count > 0 {
                let suffix = &rest[digit_count..];
                return Some(format!("Metadata/{prefix}{target_number}{suffix}"));
            }
        }
    }
    None
}

fn is_plate_json(path: &str) -> bool {
    path.starts_with("Metadata/plate_") && path.ends_with(".json")
}

fn rewrite_root_relationships(
    package: &Package,
    auxiliary_paths: &BTreeMap<String, String>,
) -> Result<Vec<Relationship>> {
    let Some(root_rels) = package.entries.get(ROOT_RELS) else {
        return Ok(Vec::new());
    };
    let root_rels =
        String::from_utf8(root_rels.clone()).context("root relationships are not UTF-8")?;
    let mut rewritten = Vec::new();

    for relationship in parse_relationships(&root_rels)? {
        if relationship.kind == "http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel" {
            continue;
        }

        let normalized_target = relationship.target.trim_start_matches('/');
        if let Some(mapped_target) = auxiliary_paths.get(normalized_target) {
            rewritten.push(Relationship {
                id: relationship.id,
                kind: relationship.kind,
                target: format!("/{mapped_target}"),
            });
        }
    }

    Ok(rewritten)
}

fn content_types_xml() -> String {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
 <Default Extension="model" ContentType="application/vnd.ms-package.3dmanufacturing-3dmodel+xml"/>
 <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
 <Default Extension="png" ContentType="image/png"/>
 <Default Extension="webp" ContentType="image/webp"/>
 <Default Extension="json" ContentType="application/json"/>
 <Default Extension="config" ContentType="application/octet-stream"/>
 <Default Extension="txt" ContentType="text/plain"/>
 <Default Extension="pdf" ContentType="application/pdf"/>
 <Default Extension="xml" ContentType="application/xml"/>
</Types>
"#
    .to_string()
}

fn root_relationships_xml(auxiliary_relationships: &[Relationship]) -> String {
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\n",
    );
    xml.push_str(" <Relationship Target=\"/3D/3dmodel.model\" Id=\"rel-1\" Type=\"http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel\"/>\n");

    for (index, relationship) in auxiliary_relationships.iter().enumerate() {
        xml.push_str(&format!(
            " <Relationship Target=\"{}\" Id=\"rel-{}\" Type=\"{}\"/>\n",
            xml_escape(&relationship.target),
            index + 2,
            xml_escape(&relationship.kind)
        ));
    }

    xml.push_str("</Relationships>\n");
    xml
}

fn model_relationships_xml(targets: &[String]) -> String {
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\n",
    );
    for (index, target) in targets.iter().enumerate() {
        xml.push_str(&format!(
            " <Relationship Target=\"{}\" Id=\"rel-{}\" Type=\"http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel\"/>\n",
            xml_escape(target),
            index + 1
        ));
    }
    xml.push_str("</Relationships>\n");
    xml
}

fn model_settings_xml(objects: &str, plates: &str, assemble_items: &str) -> String {
    let mut xml = format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<config>\n{objects}");
    if !plates.is_empty() {
        xml.push_str(plates);
    }
    if !assemble_items.is_empty() {
        xml.push_str("  <assemble>\n");
        xml.push_str(assemble_items);
        xml.push_str("  </assemble>\n");
    }
    xml.push_str("</config>\n");
    xml
}

fn metadata_filament_settings_number(path: &str) -> Option<usize> {
    let file_name = path.strip_prefix("Metadata/")?;
    if let Some(rest) = file_name.strip_prefix("filament_settings_") {
        let digits: String = rest.chars().take_while(|ch| ch.is_ascii_digit()).collect();
        if !digits.is_empty() {
            if rest[digits.len()..].starts_with(".config") {
                return digits.parse().ok();
            }
        }
    }
    None
}

fn rewrite_plate_json(
    bytes: &[u8],
    remap: &Remap,
    filament_offset: usize,
    dx: f64,
    dy: f64,
) -> Result<Vec<u8>> {
    let mut data: serde_json::Value = serde_json::from_slice(bytes)?;

    if let Some(obj) = data.as_object_mut() {
        if dx != 0.0 || dy != 0.0 {
            if let Some(bbox_all) = obj.get_mut("bbox_all").and_then(|x| x.as_array_mut()) {
                if bbox_all.len() == 4 {
                    if let (Some(x0), Some(y0), Some(x1), Some(y1)) = (
                        bbox_all[0].as_f64(),
                        bbox_all[1].as_f64(),
                        bbox_all[2].as_f64(),
                        bbox_all[3].as_f64(),
                    ) {
                        bbox_all[0] = serde_json::Value::Number(
                            serde_json::Number::from_f64(x0 + dx).unwrap(),
                        );
                        bbox_all[1] = serde_json::Value::Number(
                            serde_json::Number::from_f64(y0 + dy).unwrap(),
                        );
                        bbox_all[2] = serde_json::Value::Number(
                            serde_json::Number::from_f64(x1 + dx).unwrap(),
                        );
                        bbox_all[3] = serde_json::Value::Number(
                            serde_json::Number::from_f64(y1 + dy).unwrap(),
                        );
                    }
                }
            }
        }

        if let Some(bbox_objects) = obj.get_mut("bbox_objects").and_then(|x| x.as_array_mut()) {
            for item in bbox_objects {
                if let Some(item_obj) = item.as_object_mut() {
                    if let Some(id_val) = item_obj.get_mut("id") {
                        if let Some(id_u64) = id_val.as_u64() {
                            let source_id = id_u64 as u32;
                            if let Some(mapped) = remap.ids.get(&source_id) {
                                *id_val = serde_json::Value::Number((*mapped).into());
                            }
                        }
                    }
                    if dx != 0.0 || dy != 0.0 {
                        if let Some(bbox) = item_obj.get_mut("bbox").and_then(|x| x.as_array_mut())
                        {
                            if bbox.len() == 4 {
                                if let (Some(x0), Some(y0), Some(x1), Some(y1)) = (
                                    bbox[0].as_f64(),
                                    bbox[1].as_f64(),
                                    bbox[2].as_f64(),
                                    bbox[3].as_f64(),
                                ) {
                                    bbox[0] = serde_json::Value::Number(
                                        serde_json::Number::from_f64(x0 + dx).unwrap(),
                                    );
                                    bbox[1] = serde_json::Value::Number(
                                        serde_json::Number::from_f64(y0 + dy).unwrap(),
                                    );
                                    bbox[2] = serde_json::Value::Number(
                                        serde_json::Number::from_f64(x1 + dx).unwrap(),
                                    );
                                    bbox[3] = serde_json::Value::Number(
                                        serde_json::Number::from_f64(y1 + dy).unwrap(),
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
        if filament_offset > 0 {
            if let Some(first_ext_val) = obj.get_mut("first_extruder") {
                if let Some(ext_u64) = first_ext_val.as_u64() {
                    *first_ext_val =
                        serde_json::Value::Number((ext_u64 as usize + filament_offset).into());
                }
            }
        }
    }

    Ok(serde_json::to_vec_pretty(&data)?)
}

fn get_plate_count(package: &Package) -> usize {
    let mut plate_numbers: Vec<_> = package
        .entries
        .keys()
        .filter_map(|path| metadata_plate_number(path))
        .collect();
    plate_numbers.sort_unstable();
    plate_numbers.dedup();
    plate_numbers
        .len()
        .max(max_plate_index(package).unwrap_or(0))
}

fn get_max_identify_id(package: &Package) -> u32 {
    let Some(settings_bytes) = package.entries.get(MODEL_SETTINGS) else {
        return 0;
    };
    let Ok(settings) = String::from_utf8(settings_bytes.clone()) else {
        return 0;
    };
    let Ok(identify_id_re) =
        regex::Regex::new(r#"<metadata\b[^>]*\bkey="identify_id"[^>]*\bvalue="(\d+)""#)
    else {
        return 0;
    };

    identify_id_re
        .captures_iter(&settings)
        .filter_map(|captures| captures[1].parse::<u32>().ok())
        .max()
        .unwrap_or(0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScalingType {
    None,
    Filament,
    Plate,
}

fn is_static_filament_setting_key(key: &str) -> bool {
    [
        "nozzle_temperature",
        "nozzle_temperature_initial_layer",
        "nozzle_temperature_range_high",
        "nozzle_temperature_range_low",
        "cool_plate_temp",
        "cool_plate_temp_initial_layer",
        "eng_plate_temp",
        "eng_plate_temp_initial_layer",
        "hot_plate_temp",
        "hot_plate_temp_initial_layer",
        "supertack_plate_temp",
        "supertack_plate_temp_initial_layer",
        "textured_plate_temp",
        "textured_plate_temp_initial_layer",
        "cooling_perimeter_transition_distance",
        "cooling_slowdown_logic",
        "flush_volumes_matrix",
        "flush_volumes_vector",
        "first_x_layer_fan_speed",
        "first_x_layer_part_fan_speed",
        "full_fan_speed_layer",
        "overhang_fan_speed",
        "overhang_fan_threshold",
        "overhang_threshold_participating_cooling",
        "pre_start_fan_time",
        "pressure_advance",
        "reduce_fan_stop_start_freq",
        "required_nozzle_HRC",
        "temperature_vitrification",
        "no_slow_down_for_cooling_on_outwalls",
        "slow_down_for_layer_cooling",
        "slow_down_layer_time",
        "slow_down_min_speed",
        "hole_coef_1",
        "hole_coef_2",
        "hole_coef_3",
        "hole_limit_max",
        "hole_limit_min",
        "impact_strength_z",
        "activate_air_filtration",
        "additional_cooling_fan_speed",
        "additional_fan_full_speed_layer",
        "chamber_temperatures",
        "circle_compensation_speed",
        "close_additional_fan_first_x_layers",
        "close_fan_the_first_x_layers",
        "complete_print_exhaust_fan_speed",
        "default_filament_colour",
        "during_print_exhaust_fan_speed",
        "enable_overhang_bridge_fan",
        "enable_pressure_advance",
        "fan_cooling_layer_time",
        "fan_max_speed",
        "fan_min_speed",
        "ironing_fan_speed",
        "diameter_limit",
        "counter_coef_1",
        "counter_coef_2",
        "counter_coef_3",
        "counter_limit_max",
        "counter_limit_min",
        "override_process_overhang_speed",
        "long_retractions_when_ec",
        "retraction_distances_when_ec",
        "volumetric_speed_coefficients",
    ]
    .contains(&key)
}

fn is_printer_setting_key(key: &str, merge_printer: bool, merge_filament: bool) -> bool {
    if merge_printer {
        return false;
    }
    if merge_filament && key.starts_with("filament_") {
        return false;
    }
    key.starts_with("machine_")
        || key.starts_with("printer_")
        || key == "print_compatible_printers"
        || key == "bed_custom_model"
        || key == "bed_custom_texture"
        || key == "bed_type"
        || key == "nozzle_diameter"
        || key == "hotend_type"
        || key == "default_print_profile"
        || key == "compatible_printers"
        || key == "compatible_printers_condition"
        || key == "primary_printing_profile"
        || key == "printer_settings_id"
        || key == "print_settings_id"
        || key == "filament_settings_id"
        || key == "filament_extruder_variant"
        || key == "filament_type"
        || key == "filament_vendor"
        || key == "filament_name"
        || key == "default_filament_profile"
        || key == "filament_flow_ratio"
}

fn get_key_scaling_type(
    key: &str,
    val1: &serde_json::Value,
    n1: usize,
    p1: usize,
    val2: &serde_json::Value,
    n2: usize,
    p2: usize,
) -> ScalingType {
    if key.starts_with("filament_") || is_static_filament_setting_key(key) {
        return ScalingType::Filament;
    }
    if key == "wipe_tower_x" || key == "wipe_tower_y" {
        return ScalingType::Plate;
    }

    let is_fil1 = val1
        .as_array()
        .map_or(false, |a| n1 > 0 && a.len() % n1 == 0);
    let is_fil2 = val2
        .as_array()
        .map_or(false, |a| n2 > 0 && a.len() % n2 == 0);
    let is_plate1 = val1
        .as_array()
        .map_or(false, |a| p1 > 0 && a.len() % p1 == 0);
    let is_plate2 = val2
        .as_array()
        .map_or(false, |a| p2 > 0 && a.len() % p2 == 0);

    let fil_match = is_fil1
        && is_fil2
        && (val1.as_array().unwrap().len() / n1 == val2.as_array().unwrap().len() / n2);
    let plate_match = is_plate1
        && is_plate2
        && (val1.as_array().unwrap().len() / p1 == val2.as_array().unwrap().len() / p2);

    if fil_match && plate_match {
        if n1 != p1 {
            let len = val1.as_array().unwrap().len();
            if len % n1 == 0 && len % p1 != 0 {
                return ScalingType::Filament;
            }
            if len % p1 == 0 && len % n1 != 0 {
                return ScalingType::Plate;
            }
        }
        if n2 != p2 {
            let len = val2.as_array().unwrap().len();
            if len % n2 == 0 && len % p2 != 0 {
                return ScalingType::Filament;
            }
            if len % p2 == 0 && len % n2 != 0 {
                return ScalingType::Plate;
            }
        }
        return ScalingType::Filament;
    }

    if fil_match {
        return ScalingType::Filament;
    }
    if plate_match {
        return ScalingType::Plate;
    }

    ScalingType::None
}

fn get_single_key_scaling_type(
    key: &str,
    val: &serde_json::Value,
    n: usize,
    p: usize,
) -> ScalingType {
    if key.starts_with("filament_") || is_static_filament_setting_key(key) {
        return ScalingType::Filament;
    }
    if key == "wipe_tower_x" || key == "wipe_tower_y" {
        return ScalingType::Plate;
    }

    if let Some(a) = val.as_array() {
        let is_fil = n > 0 && a.len() % n == 0;
        let is_plate = p > 0 && a.len() % p == 0;
        if is_fil && is_plate {
            if n != p {
                if a.len() % n == 0 && a.len() % p != 0 {
                    return ScalingType::Filament;
                }
                if a.len() % p == 0 && a.len() % n != 0 {
                    return ScalingType::Plate;
                }
            }
            return ScalingType::Filament;
        }
        if is_fil {
            return ScalingType::Filament;
        }
        if is_plate {
            return ScalingType::Plate;
        }
    }
    ScalingType::None
}

fn merge_project_settings(
    master_bytes: &[u8],
    next_bytes: &[u8],
    p1: usize,
    p2: usize,
    merge_printer: bool,
    merge_filament: bool,
) -> Result<Vec<u8>> {
    let mut master: serde_json::Value = serde_json::from_slice(master_bytes)?;
    let next: serde_json::Value = serde_json::from_slice(next_bytes)?;

    let n1 = get_filament_count(&master);
    let n2 = get_filament_count(&next);

    if n1 == 0 || n2 == 0 {
        return Ok(master_bytes.to_vec());
    }

    if let (Some(m_obj), Some(n_obj)) = (master.as_object_mut(), next.as_object()) {
        for (k, v) in n_obj {
            // Skip printer/machine settings - keep only from first input
            if is_printer_setting_key(&k, merge_printer, merge_filament) {
                continue;
            }

            // Special presets merging
            if k == "inherits_group" || k == "different_settings_to_system" {
                if let (Some(a1), Some(a2)) =
                    (m_obj.get(k).and_then(|x| x.as_array()), v.as_array())
                {
                    if a1.len() == n1 + 2 && a2.len() == n2 + 2 {
                        let mut combined = Vec::new();
                        combined.push(a1[0].clone());
                        combined.extend(a1[1..=n1].iter().cloned());
                        combined.extend(a2[1..=n2].iter().cloned());
                        combined.push(a1[n1 + 1].clone());
                        m_obj.insert(k.clone(), serde_json::Value::Array(combined));
                        continue;
                    }
                }
            }

            let sc_type = if let Some(m_v) = m_obj.get(k) {
                get_key_scaling_type(k, m_v, n1, p1, v, n2, p2)
            } else {
                get_single_key_scaling_type(k, v, n2, p2)
            };

            match sc_type {
                ScalingType::Filament => {
                    if k == "flush_volumes_matrix" {
                        if let (Some(m1), Some(m2)) =
                            (m_obj.get(k).and_then(|x| x.as_array()), v.as_array())
                        {
                            let merged = merge_flush_matrices(m1, n1, m2, n2);
                            m_obj.insert(k.clone(), serde_json::Value::Array(merged));
                        }
                    } else if let Some(a2) = v.as_array() {
                        if let Some(m_val) = m_obj.get(k) {
                            if let Some(a1) = m_val.as_array() {
                                let mut combined = a1.clone();
                                if k == "filament_self_index" {
                                    let shifted: Vec<serde_json::Value> = a2
                                        .iter()
                                        .map(|item| {
                                            if let Some(s) = item.as_str() {
                                                if let Ok(idx) = s.parse::<usize>() {
                                                    if idx > 0 {
                                                        return serde_json::Value::String(
                                                            (idx + n1).to_string(),
                                                        );
                                                    }
                                                }
                                            }
                                            item.clone()
                                        })
                                        .collect();
                                    combined.extend(shifted);
                                } else {
                                    combined.extend(a2.clone());
                                }
                                m_obj.insert(k.clone(), serde_json::Value::Array(combined));
                            }
                        } else {
                            let factor = a2.len() / n2;
                            let pad_len = factor * n1;
                            let mut combined =
                                vec![serde_json::Value::String("nil".to_string()); pad_len];
                            combined.extend(a2.clone());
                            m_obj.insert(k.clone(), serde_json::Value::Array(combined));
                        }
                    } else {
                        if !m_obj.contains_key(k) {
                            m_obj.insert(k.clone(), v.clone());
                        }
                    }
                }
                ScalingType::Plate => {
                    if let Some(a2) = v.as_array() {
                        if let Some(m_val) = m_obj.get(k) {
                            if let Some(a1) = m_val.as_array() {
                                let mut combined = a1.clone();
                                combined.extend(a2.clone());
                                m_obj.insert(k.clone(), serde_json::Value::Array(combined));
                            }
                        } else {
                            let factor = a2.len() / p2;
                            let pad_len = factor * p1;
                            let mut combined =
                                vec![serde_json::Value::String("nil".to_string()); pad_len];
                            combined.extend(a2.clone());
                            m_obj.insert(k.clone(), serde_json::Value::Array(combined));
                        }
                    } else {
                        if !m_obj.contains_key(k) {
                            m_obj.insert(k.clone(), v.clone());
                        }
                    }
                }
                ScalingType::None => {
                    if !m_obj.contains_key(k) {
                        m_obj.insert(k.clone(), v.clone());
                    }
                }
            }
        }
        let keys_to_pad: Vec<(String, ScalingType)> = m_obj
            .keys()
            .filter(|k| {
                // Include keys that are NOT in next input OR are printer settings we're skipping
                !n_obj.contains_key(*k) || is_printer_setting_key(k, merge_printer, merge_filament)
            })
            .filter_map(|k| {
                let sc_type = get_single_key_scaling_type(k, m_obj.get(k).unwrap(), n1, p1);
                if sc_type != ScalingType::None {
                    Some((k.clone(), sc_type))
                } else {
                    None
                }
            })
            .collect();

        for key in ["inherits_group", "different_settings_to_system"] {
            if !n_obj.contains_key(key) {
                if let Some(values) = m_obj.get_mut(key).and_then(|x| x.as_array_mut()) {
                    if values.len() == n1 + 2 {
                        let insert_at = values.len() - 1;
                        values.splice(
                            insert_at..insert_at,
                            vec![serde_json::Value::String(String::new()); n2],
                        );
                    }
                }
            }
        }

        for (k, sc_type) in keys_to_pad {
            if let Some(a1) = m_obj.get(&k).and_then(|x| x.as_array()) {
                match sc_type {
                    ScalingType::Filament => {
                        let factor = a1.len() / n1;
                        let pad_len = factor * n2;
                        // Special handling for filament_settings_id: use second input's values if available
                        // to match stock presets for the printer
                        let mut combined = a1.clone();
                        if k == "filament_settings_id" {
                            if let Some(a2) = n_obj.get(k.as_str()).and_then(|x| x.as_array()) {
                                // Use second input's values cyclically for padding
                                for i in 0..pad_len {
                                    let src_idx = i % a2.len();
                                    combined.push(a2[src_idx].clone());
                                }
                            } else {
                                // Fallback: replicate first input's pattern
                                for i in 0..pad_len {
                                    let src_idx = (i / factor) % n1 * factor + (i % factor);
                                    combined.push(a1[src_idx].clone());
                                }
                            }
                        } else {
                            // Replicate the pattern from existing filaments instead of using "nil"
                            for i in 0..pad_len {
                                // For each padding filament, copy the pattern from corresponding original filament
                                let src_idx = (i / factor) % n1 * factor + (i % factor);
                                combined.push(a1[src_idx].clone());
                            }
                        }
                        m_obj.insert(k, serde_json::Value::Array(combined));
                    }
                    ScalingType::Plate => {
                        let factor = a1.len() / p1;
                        let pad_len = factor * p2;
                        let mut combined = a1.clone();
                        for i in 0..pad_len {
                            let src_idx = (i / factor) % p1 * factor + (i % factor);
                            combined.push(a1[src_idx].clone());
                        }
                        m_obj.insert(k, serde_json::Value::Array(combined));
                    }
                    ScalingType::None => {}
                }
            }
        }
    }

    Ok(serde_json::to_vec_pretty(&master)?)
}

fn get_filament_count(val: &serde_json::Value) -> usize {
    val.get("filament_colour")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0)
}

fn dedupe_merged_filaments(entries: &mut BTreeMap<String, Vec<u8>>) -> Result<()> {
    let Some(project_bytes) = entries.get("Metadata/project_settings.config").cloned() else {
        return Ok(());
    };
    let mut project: serde_json::Value = serde_json::from_slice(&project_bytes)?;
    let old_count = get_filament_count(&project);
    if old_count <= 1 {
        return Ok(());
    }

    let profiles = filament_dedupe_profiles(&project, old_count)?;
    let mut seen = BTreeMap::new();
    let mut representatives = Vec::new();
    let mut old_to_new = vec![0; old_count + 1];

    for old_index in 1..=old_count {
        let profile = &profiles[old_index - 1];
        if let Some(&new_index) = seen.get(profile) {
            old_to_new[old_index] = new_index;
        } else {
            let new_index = representatives.len() + 1;
            seen.insert(profile.clone(), new_index);
            representatives.push(old_index);
            old_to_new[old_index] = new_index;
        }
    }

    if representatives.len() == old_count {
        return Ok(());
    }

    dedupe_project_settings_arrays(&mut project, old_count, &representatives, &old_to_new);
    entries.insert(
        "Metadata/project_settings.config".to_string(),
        serde_json::to_vec_pretty(&project)?,
    );
    rewrite_deduped_filament_references(entries, &project, old_count, &old_to_new, &representatives)?;

    Ok(())
}

fn filament_dedupe_profiles(
    project: &serde_json::Value,
    filament_count: usize,
) -> Result<Vec<String>> {
    const PROFILE_KEYS: &[&str] = &[
        "filament_colour",
        "filament_type",
        "filament_vendor",
        "filament_density",
        "filament_diameter",
        "filament_flow_ratio",
        "filament_shrink",
        "filament_max_volumetric_speed",
        "filament_retraction_length",
        "filament_retraction_speed",
        "filament_deretraction_speed",
        "filament_z_hop",
        "filament_wipe",
        "filament_wipe_distance",
        "nozzle_temperature",
        "nozzle_temperature_initial_layer",
        "cool_plate_temp",
        "cool_plate_temp_initial_layer",
        "eng_plate_temp",
        "eng_plate_temp_initial_layer",
        "hot_plate_temp",
        "hot_plate_temp_initial_layer",
        "supertack_plate_temp",
        "supertack_plate_temp_initial_layer",
        "textured_plate_temp",
        "textured_plate_temp_initial_layer",
        "temperature_vitrification",
        "fan_min_speed",
        "fan_max_speed",
        "slow_down_for_layer_cooling",
        "slow_down_layer_time",
        "slow_down_min_speed",
    ];

    let mut profiles = Vec::with_capacity(filament_count);
    for old_index in 1..=filament_count {
        let mut profile = Vec::new();
        for key in PROFILE_KEYS {
            if let Some(value) = project.get(*key) {
                profile.push((
                    *key,
                    filament_profile_value(value, old_index, filament_count),
                ));
            }
        }
        profiles.push(serde_json::to_string(&profile)?);
    }
    Ok(profiles)
}

fn filament_profile_value(
    value: &serde_json::Value,
    old_index: usize,
    filament_count: usize,
) -> serde_json::Value {
    let Some(values) = value.as_array() else {
        return value.clone();
    };
    if values.len() == filament_count {
        return values[old_index - 1].clone();
    }
    if filament_count > 0 && values.len() % filament_count == 0 {
        let factor = values.len() / filament_count;
        let start = (old_index - 1) * factor;
        let end = start + factor;
        return serde_json::Value::Array(values[start..end].to_vec());
    }
    value.clone()
}

fn dedupe_project_settings_arrays(
    project: &mut serde_json::Value,
    old_count: usize,
    representatives: &[usize],
    old_to_new: &[usize],
) {
    let Some(settings) = project.as_object_mut() else {
        return;
    };

    for (key, value) in settings.iter_mut() {
        let Some(values) = value.as_array() else {
            continue;
        };
        if values.is_empty() || old_count == 0 {
            continue;
        }

        let is_filament_scaled_key = key.starts_with("filament_")
            || is_static_filament_setting_key(key)
            || key == "inherits_group"
            || key == "different_settings_to_system";
        if !is_filament_scaled_key {
            continue;
        }

        let compacted = if key == "flush_volumes_matrix" && values.len() == old_count * old_count {
            let new_count = representatives.len();
            let mut out = Vec::with_capacity(new_count * new_count);
            for &old_row in representatives {
                for &old_col in representatives {
                    out.push(values[(old_row - 1) * old_count + (old_col - 1)].clone());
                }
            }
            Some(out)
        } else if (key == "inherits_group" || key == "different_settings_to_system")
            && values.len() == old_count + 2
        {
            let mut out = Vec::with_capacity(representatives.len() + 2);
            out.push(values[0].clone());
            out.extend(representatives.iter().map(|old| values[*old].clone()));
            out.push(values[old_count + 1].clone());
            Some(out)
        } else if values.len() == old_count {
            Some(
                representatives
                    .iter()
                    .map(|old| values[*old - 1].clone())
                    .collect(),
            )
        } else if values.len() % old_count == 0 {
            let factor = values.len() / old_count;
            let mut out = Vec::with_capacity(representatives.len() * factor);
            for &old in representatives {
                let start = (old - 1) * factor;
                out.extend(values[start..start + factor].iter().cloned());
            }
            if key == "filament_self_index" {
                for val in &mut out {
                    if let Some(s) = val.as_str() {
                        if let Ok(idx) = s.parse::<usize>() {
                            if idx > 0 && idx < old_to_new.len() {
                                let new_idx = old_to_new[idx];
                                *val = serde_json::Value::String(new_idx.to_string());
                            }
                        }
                    }
                }
            }
            Some(out)
        } else {
            None
        };

        if let Some(compacted) = compacted {
            *value = serde_json::Value::Array(compacted);
        }
    }
}

fn rewrite_deduped_filament_references(
    entries: &mut BTreeMap<String, Vec<u8>>,
    project: &serde_json::Value,
    old_count: usize,
    old_to_new: &[usize],
    representatives: &[usize],
) -> Result<()> {
    // Collect and remove all old filament settings files
    let mut old_settings = BTreeMap::new();
    let paths: Vec<_> = entries.keys().cloned().collect();
    for path in paths {
        if let Some(old_idx) = metadata_filament_settings_number(&path) {
            if let Some(bytes) = entries.remove(&path) {
                old_settings.insert(old_idx, bytes);
            }
        }
    }

    // Write back only the representative files under their new indices
    let new_filament_ids = project
        .get("filament_settings_id")
        .and_then(|v| v.as_array());

    for (old_idx, bytes) in old_settings {
        if !representatives.contains(&old_idx) {
            continue;
        }

        // Try to find the new index K by matching preset ID to avoid index mismatch
        let mut new_idx = None;
        if let Ok(config_json) = serde_json::from_slice::<serde_json::Value>(&bytes) {
            let preset_id = config_json
                .get("filament_settings_id")
                .and_then(|v| v.as_array())
                .and_then(|arr| arr.first())
                .and_then(|v| v.as_str())
                .or_else(|| {
                    config_json.get("name").and_then(|v| v.as_str())
                });

            if let Some(preset_id) = preset_id {
                if let Some(ids) = new_filament_ids {
                    if let Some(pos) = ids.iter().position(|id| id.as_str() == Some(preset_id)) {
                        new_idx = Some(pos + 1);
                    }
                }
            }
        }

        // Fallback to old mapping if not found by ID
        let new_idx = new_idx.unwrap_or_else(|| old_to_new[old_idx]);

        if new_idx > 0 {
            let new_path = format!("Metadata/filament_settings_{}.config", new_idx);
            entries.insert(new_path, bytes);
        }
    }

    let paths: Vec<_> = entries.keys().cloned().collect();
    for path in paths {
        if path == MODEL || is_object_model(&path) {
            let Some(bytes) = entries.get(&path).cloned() else {
                continue;
            };
            let xml = String::from_utf8(bytes).with_context(|| format!("{path} is not UTF-8"))?;
            let mut rewritten = remap_filament_xml_attr(&xml, "face_property", old_to_new)?;
            rewritten = remap_filament_xml_attr(&rewritten, "paint_supports", old_to_new)?;
            entries.insert(path, rewritten.into_bytes());
        } else if path == MODEL_SETTINGS {
            let Some(bytes) = entries.get(&path).cloned() else {
                continue;
            };
            let xml = String::from_utf8(bytes).context("model_settings.config is not UTF-8")?;
            let rewritten = rewrite_deduped_model_settings(&xml, old_count, old_to_new)?;
            entries.insert(path, rewritten.into_bytes());
        } else if path == "Metadata/filament_sequence.json" {
            let Some(bytes) = entries.get(&path).cloned() else {
                continue;
            };
            let mut sequence: serde_json::Value = serde_json::from_slice(&bytes)?;
            remap_filament_sequence_value(&mut sequence, old_to_new);
            entries.insert(path, serde_json::to_vec(&sequence)?);
        } else if is_plate_json(&path) {
            let Some(bytes) = entries.get(&path).cloned() else {
                continue;
            };
            let mut plate_json: serde_json::Value = serde_json::from_slice(&bytes)?;
            remap_plate_json_filament_indices(&mut plate_json, old_to_new);
            entries.insert(path, serde_json::to_vec_pretty(&plate_json)?);
        }
    }
    Ok(())
}

fn remap_filament_xml_attr(xml: &str, attr: &str, old_to_new: &[usize]) -> Result<String> {
    let re = regex::Regex::new(&format!(r#"(\b{}=")(\d+)(")"#, regex::escape(attr)))?;
    Ok(re
        .replace_all(xml, |captures: &regex::Captures<'_>| {
            let old: usize = captures[2].parse().unwrap_or(0);
            if let Some(new) = remap_filament_index(old, old_to_new) {
                format!("{}{}{}", &captures[1], new, &captures[3])
            } else {
                captures[0].to_string()
            }
        })
        .into_owned())
}

fn rewrite_deduped_model_settings(
    xml: &str,
    old_count: usize,
    old_to_new: &[usize],
) -> Result<String> {
    let extruder_re =
        regex::Regex::new(r#"(<metadata\b[^>]*\bkey="extruder"[^>]*\bvalue=")(\d+)(")"#)?;
    let mut rewritten = extruder_re
        .replace_all(xml, |captures: &regex::Captures<'_>| {
            let old: usize = captures[2].parse().unwrap_or(0);
            if let Some(new) = remap_filament_index(old, old_to_new) {
                format!("{}{}{}", &captures[1], new, &captures[3])
            } else {
                captures[0].to_string()
            }
        })
        .into_owned();

    let maps_re =
        regex::Regex::new(r#"(<metadata\b[^>]*\bkey="filament_maps"[^>]*\bvalue=")([^"]*)(")"#)?;
    rewritten = maps_re
        .replace_all(&rewritten, |captures: &regex::Captures<'_>| {
            let value = compact_filament_map_value(&captures[2], old_count, old_to_new, false)
                .unwrap_or_else(|| captures[2].to_string());
            format!("{}{}{}", &captures[1], value, &captures[3])
        })
        .into_owned();

    let volumes_re = regex::Regex::new(
        r#"(<metadata\b[^>]*\bkey="filament_volume_maps"[^>]*\bvalue=")([^"]*)(")"#,
    )?;
    rewritten = volumes_re
        .replace_all(&rewritten, |captures: &regex::Captures<'_>| {
            let value = compact_filament_map_value(&captures[2], old_count, old_to_new, true)
                .unwrap_or_else(|| captures[2].to_string());
            format!("{}{}{}", &captures[1], value, &captures[3])
        })
        .into_owned();

    Ok(rewritten)
}

fn remap_filament_index(old: usize, old_to_new: &[usize]) -> Option<usize> {
    if old == 0 {
        Some(0)
    } else {
        old_to_new.get(old).copied().filter(|new| *new > 0)
    }
}

fn compact_filament_map_value(
    value: &str,
    old_count: usize,
    old_to_new: &[usize],
    sum_values: bool,
) -> Option<String> {
    let parts: Vec<_> = value.split_whitespace().collect();
    if parts.len() != old_count {
        return None;
    }
    let new_count = old_to_new.iter().copied().max().unwrap_or(0);
    if new_count == 0 {
        return None;
    }

    if sum_values {
        let mut numeric = vec![0.0; new_count];
        for (old_zero, part) in parts.iter().enumerate() {
            let parsed = part.parse::<f64>().ok()?;
            let new_index = old_to_new[old_zero + 1];
            numeric[new_index - 1] += parsed;
        }
        Some(
            numeric
                .into_iter()
                .map(format_compact_number)
                .collect::<Vec<_>>()
                .join(" "),
        )
    } else {
        let mut compacted = vec!["0".to_string(); new_count];
        for (old_zero, part) in parts.iter().enumerate() {
            let new_index = old_to_new[old_zero + 1];
            let target = &mut compacted[new_index - 1];
            if target == "0" && *part != "0" {
                *target = (*part).to_string();
            }
        }
        Some(compacted.join(" "))
    }
}

fn format_compact_number(value: f64) -> String {
    let formatted = format!("{value:.9}");
    let trimmed = formatted.trim_end_matches('0').trim_end_matches('.');
    if trimmed.is_empty() {
        "0".to_string()
    } else {
        trimmed.to_string()
    }
}

fn remap_filament_sequence_value(value: &mut serde_json::Value, old_to_new: &[usize]) {
    match value {
        serde_json::Value::Array(values) => {
            for item in values {
                remap_filament_sequence_value(item, old_to_new);
            }
        }
        serde_json::Value::Object(values) => {
            for item in values.values_mut() {
                remap_filament_sequence_value(item, old_to_new);
            }
        }
        serde_json::Value::Number(number) => {
            if let Some(old) = number.as_u64().map(|v| v as usize) {
                if let Some(new) = remap_filament_index(old, old_to_new) {
                    *value = serde_json::Value::Number(new.into());
                }
            }
        }
        serde_json::Value::String(text) => {
            if let Ok(old) = text.parse::<usize>() {
                if let Some(new) = remap_filament_index(old, old_to_new) {
                    *text = new.to_string();
                }
            }
        }
        _ => {}
    }
}

fn remap_plate_json_filament_indices(value: &mut serde_json::Value, old_to_new: &[usize]) {
    match value {
        serde_json::Value::Array(values) => {
            for item in values {
                remap_plate_json_filament_indices(item, old_to_new);
            }
        }
        serde_json::Value::Object(values) => {
            for (key, item) in values {
                if key == "first_extruder" || key == "filament_ids" {
                    remap_filament_sequence_value(item, old_to_new);
                } else {
                    remap_plate_json_filament_indices(item, old_to_new);
                }
            }
        }
        _ => {}
    }
}

fn merge_flush_matrices(
    m1: &[serde_json::Value],
    n1: usize,
    m2: &[serde_json::Value],
    n2: usize,
) -> Vec<serde_json::Value> {
    let size = n1 + n2;
    let mut out = vec![serde_json::Value::String("80".to_string()); size * size];
    for r in 0..size {
        for c in 0..size {
            if r < n1 && c < n1 {
                if r * n1 + c < m1.len() {
                    out[r * size + c] = m1[r * n1 + c].clone();
                }
            } else if r >= n1 && c >= n1 {
                let r2 = r - n1;
                let c2 = c - n1;
                if r2 * n2 + c2 < m2.len() {
                    out[r * size + c] = m2[r2 * n2 + c2].clone();
                }
            } else {
                if r == c {
                    out[r * size + c] = serde_json::Value::String("0".to_string());
                } else {
                    out[r * size + c] = serde_json::Value::String("80".to_string());
                }
            }
        }
    }
    out
}

fn merge_filament_sequence(
    master_bytes: &[u8],
    next_bytes: &[u8],
    plate_offset: usize,
) -> Result<Vec<u8>> {
    let mut master: serde_json::Value = serde_json::from_slice(master_bytes)?;
    let next: serde_json::Value = serde_json::from_slice(next_bytes)?;

    if let (Some(m_obj), Some(n_obj)) = (master.as_object_mut(), next.as_object()) {
        for (k, v) in n_obj {
            if let Some(rest) = k.strip_prefix("plate_") {
                if let Ok(idx) = rest.parse::<usize>() {
                    let new_key = format!("plate_{}", idx + plate_offset);
                    m_obj.insert(new_key, v.clone());
                } else {
                    m_obj.insert(k.clone(), v.clone());
                }
            } else {
                m_obj.insert(k.clone(), v.clone());
            }
        }
    }

    Ok(serde_json::to_vec(&master)?)
}

fn get_object_plate_map(package: &Package) -> Result<BTreeMap<u32, usize>> {
    let mut map = BTreeMap::new();
    if let Some(settings_bytes) = package.entries.get(MODEL_SETTINGS) {
        let settings = String::from_utf8(settings_bytes.clone())?;
        for plate in config_plate_elements(&settings)? {
            let plater_id_re =
                regex::Regex::new(r#"<metadata\b[^>]*\bkey="plater_id"[^>]*\bvalue="(\d+)""#)?;
            let plater_id = if let Some(captures) = plater_id_re.captures(&plate) {
                captures[1].parse::<usize>()?
            } else {
                1
            };
            let object_id_re =
                regex::Regex::new(r#"<metadata\b[^>]*\bkey="object_id"[^>]*\bvalue="(\d+)""#)?;
            for captures in object_id_re.captures_iter(&plate) {
                let obj_id = captures[1].parse::<u32>()?;
                map.insert(obj_id, plater_id);
            }
        }
    }
    Ok(map)
}

fn get_plate_shift(
    index: usize,
    source_plate: usize,
    plate_offsets: &[usize],
    source_plate_count: usize,
    total_plate_count: usize,
) -> (f64, f64) {
    let p_before = plate_offsets[index];
    let p_target = p_before + source_plate;

    let (col_target, row_target) = plate_position(p_target, total_plate_count);
    let (col_source, row_source) = plate_position(source_plate, source_plate_count);

    let dx = (col_target as f64 - col_source as f64) * 300.0;
    let dy = (row_target as f64 - row_source as f64) * -320.0;

    (dx, dy)
}

fn plate_position(plate: usize, plate_count: usize) -> (usize, usize) {
    let columns = plate_grid_columns(plate_count);
    ((plate - 1) % columns, (plate - 1) / columns)
}

fn plate_grid_columns(plate_count: usize) -> usize {
    match plate_count {
        0 | 1 => 1,
        2 => 2,
        count => (count as f64).sqrt().ceil().max(3.0) as usize,
    }
}

fn rewrite_build_item_transforms(
    xml: &str,
    index: usize,
    plate_offsets: &[usize],
    source_plate_count: usize,
    total_plate_count: usize,
    object_to_plate: &BTreeMap<u32, usize>,
) -> Result<String> {
    let item_re =
        regex::Regex::new(r#"(<item\b[^>]*\bobjectid=")(\d+)("[^>]*\btransform=")([^"]+)(")"#)?;
    let rewritten = item_re
        .replace_all(xml, |captures: &regex::Captures<'_>| {
            let object_id: u32 = captures[2].parse().unwrap();
            if let Some(&plate_id) = object_to_plate.get(&object_id) {
                let (dx, dy) = get_plate_shift(
                    index,
                    plate_id,
                    plate_offsets,
                    source_plate_count,
                    total_plate_count,
                );
                if dx != 0.0 || dy != 0.0 {
                    let orig_transform = &captures[4];
                    if let Some(new_transform) = shift_transform_matrix(orig_transform, dx, dy) {
                        return format!(
                            "{}{}{}{}{}",
                            &captures[1], object_id, &captures[3], new_transform, &captures[5]
                        );
                    }
                }
            }
            captures[0].to_string()
        })
        .into_owned();

    Ok(rewritten)
}

fn shift_transform_matrix(matrix_str: &str, dx: f64, dy: f64) -> Option<String> {
    let parts: Vec<&str> = matrix_str.split_whitespace().collect();
    if parts.len() < 12 {
        return None;
    }
    let mut numbers: Vec<f64> = parts
        .iter()
        .map(|p| p.parse::<f64>().unwrap_or(0.0))
        .collect();
    numbers[9] += dx;
    numbers[10] += dy;
    let formatted: Vec<String> = numbers
        .iter()
        .map(|n| {
            format!("{:.9}", n)
                .trim_end_matches('0')
                .trim_end_matches('.')
                .to_string()
        })
        .collect();
    Some(formatted.join(" "))
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_too_few_inputs() {
        let err = merge_files(
            &[],
            Path::new("out.3mf"),
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
        )
        .unwrap_err();
        assert!(err.to_string().contains("at least two input"));
    }

    #[test]
    fn rewrites_root_auxiliary_relationships() {
        let mut package = Package {
            entries: BTreeMap::new(),
        };
        package.entries.insert(
            ROOT_RELS.to_string(),
            br#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
 <Relationship Target="/3D/3dmodel.model" Id="rel-1" Type="http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel"/>
 <Relationship Target="/Auxiliaries/.thumbnails/thumbnail_3mf.png" Id="rel-2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/thumbnail"/>
</Relationships>"#
                .to_vec(),
        );
        let auxiliary_paths = BTreeMap::from([(
            "Auxiliaries/.thumbnails/thumbnail_3mf.png".to_string(),
            "MergedInputs/input-002/Auxiliaries/.thumbnails/thumbnail_3mf.png".to_string(),
        )]);

        let relationships = rewrite_root_relationships(&package, &auxiliary_paths).unwrap();

        assert_eq!(relationships.len(), 1);
        assert_eq!(
            relationships[0].target,
            "/MergedInputs/input-002/Auxiliaries/.thumbnails/thumbnail_3mf.png"
        );
    }

    #[test]
    fn promotes_later_input_plate_metadata_after_existing_indices() {
        let mut package = Package {
            entries: BTreeMap::new(),
        };
        package
            .entries
            .insert("Metadata/plate_1.png".into(), vec![]);
        package.entries.insert("Metadata/top_1.png".into(), vec![]);
        package.entries.insert("Metadata/pick_1.png".into(), vec![]);
        package
            .entries
            .insert("Metadata/plate_2.json".into(), vec![]);

        let plan = plan_plate_promotions(1, &package, 8).unwrap();

        assert_eq!(plan["Metadata/plate_1.png"], "Metadata/plate_8.png");
        assert_eq!(plan["Metadata/top_1.png"], "Metadata/top_8.png");
        assert_eq!(plan["Metadata/pick_1.png"], "Metadata/pick_8.png");
        assert_eq!(plan["Metadata/plate_2.json"], "Metadata/plate_9.json");
    }

    #[test]
    fn uses_bambu_square_plate_layout_columns() {
        assert_eq!(plate_grid_columns(1), 1);
        assert_eq!(plate_grid_columns(2), 2);
        for count in 3..=9 {
            assert_eq!(plate_grid_columns(count), 3);
        }
        for count in 10..=16 {
            assert_eq!(plate_grid_columns(count), 4);
        }
        for count in 17..=25 {
            assert_eq!(plate_grid_columns(count), 5);
        }
        for count in 26..=36 {
            assert_eq!(plate_grid_columns(count), 6);
        }
    }

    #[test]
    fn shifts_between_source_and_merged_bambu_plate_layouts() {
        let plate_offsets = vec![0, 7];

        // Source plate 2 in a 6-plate input is col 1,row 0.
        // Merged plate 9 in a 13-plate output is col 0,row 2.
        assert_eq!(
            get_plate_shift(1, 2, &plate_offsets, 6, 13),
            (-300.0, -640.0)
        );
    }

    #[test]
    fn shifts_first_input_when_merged_layout_changes() {
        let plate_offsets = vec![0, 6];

        // Source plate 4 in a 6-plate input is col 0,row 1.
        // Merged plate 4 in a 12-plate output is col 3,row 0.
        assert_eq!(get_plate_shift(0, 4, &plate_offsets, 6, 12), (900.0, 320.0));
    }

    #[test]
    fn dedupes_matching_filaments_and_rewrites_references() {
        let mut entries = BTreeMap::new();
        entries.insert(
            "Metadata/project_settings.config".to_string(),
            br##"{
  "filament_colour": ["#112233", "#112233", "#445566"],
  "filament_type": ["PLA", "PLA", "PLA"],
  "filament_vendor": ["Generic", "Generic", "Generic"],
  "filament_flow_ratio": ["1", "1", "1"],
  "filament_settings_id": ["Custom first", "Custom second", "Custom third"],
  "nozzle_temperature": ["220", "220", "220"],
  "inherits_group": ["process", "first", "second", "third", "printer"],
  "different_settings_to_system": ["process", "first", "second", "third", "printer"],
  "flush_volumes_matrix": ["0", "12", "34", "0", "0", "0", "0", "0", "0"],
  "filament_self_index": ["1", "1", "2", "2", "3", "3"],
  "printable_area": ["0x0", "256x0", "256x256", "0x256"]
}"##
            .to_vec(),
        );
        entries.insert(
            MODEL.to_string(),
            br#"<model><resources><object id="1"><mesh><triangles><triangle face_property="3" paint_supports="3"/></triangles></mesh></object></resources><build/></model>"#.to_vec(),
        );
        entries.insert(
            "3D/Objects/object_1.model".to_string(),
            br#"<model><resources><object id="1"><mesh><triangles><triangle face_property="3" paint_supports="3"/></triangles></mesh></object></resources><build/></model>"#.to_vec(),
        );
        entries.insert(
            MODEL_SETTINGS.to_string(),
            br#"<config>
  <object id="1"><part id="1"><metadata key="extruder" value="3"/></part></object>
  <plate>
    <metadata key="filament_maps" value="0 0 1"/>
    <metadata key="filament_volume_maps" value="2.5 3.5 4.5"/>
  </plate>
</config>"#
                .to_vec(),
        );
        entries.insert(
            "Metadata/plate_1.json".to_string(),
            br#"{"first_extruder":3,"filament_ids":[1,3]}"#.to_vec(),
        );
        entries.insert(
            "Metadata/filament_sequence.json".to_string(),
            br#"{"plate_1":{"sequence":[3],"nozzle_sequence":["3"]}}"#.to_vec(),
        );
        entries.insert(
            "Metadata/filament_settings_2.config".to_string(),
            br#"{"filament_settings_id":["Custom second"],"name":"Custom second"}"#.to_vec(),
        );
        entries.insert(
            "Metadata/filament_settings_3.config".to_string(),
            br#"{"filament_settings_id":["Custom third"],"name":"Custom third"}"#.to_vec(),
        );

        dedupe_merged_filaments(&mut entries).unwrap();

        let project: serde_json::Value =
            serde_json::from_slice(&entries["Metadata/project_settings.config"]).unwrap();
        assert_eq!(get_filament_count(&project), 2);
        assert_eq!(project["filament_settings_id"].as_array().unwrap().len(), 2);
        assert_eq!(project["inherits_group"].as_array().unwrap().len(), 4);
        assert_eq!(project["flush_volumes_matrix"].as_array().unwrap().len(), 4);
        assert_eq!(
            project["filament_self_index"].as_array().unwrap(),
            &vec![
                serde_json::json!("1"),
                serde_json::json!("1"),
                serde_json::json!("2"),
                serde_json::json!("2")
            ]
        );
        assert_eq!(project["printable_area"].as_array().unwrap().len(), 4);

        let model = String::from_utf8(entries[MODEL].clone()).unwrap();
        assert!(model.contains(r#"face_property="2""#));
        assert!(model.contains(r#"paint_supports="2""#));
        let object = String::from_utf8(entries["3D/Objects/object_1.model"].clone()).unwrap();
        assert!(object.contains(r#"face_property="2""#));
        assert!(object.contains(r#"paint_supports="2""#));

        let model_settings = String::from_utf8(entries[MODEL_SETTINGS].clone()).unwrap();
        assert!(model_settings.contains(r#"key="extruder" value="2""#));
        assert!(model_settings.contains(r#"key="filament_maps" value="0 1""#));
        assert!(model_settings.contains(r#"key="filament_volume_maps" value="6 4.5""#));

        let plate_json: serde_json::Value =
            serde_json::from_slice(&entries["Metadata/plate_1.json"]).unwrap();
        assert_eq!(plate_json["first_extruder"].as_u64(), Some(2));
        assert_eq!(plate_json["filament_ids"].as_array().unwrap(), &vec![serde_json::json!(1), serde_json::json!(2)]);

        let sequence: serde_json::Value =
            serde_json::from_slice(&entries["Metadata/filament_sequence.json"]).unwrap();
        assert_eq!(sequence["plate_1"]["sequence"][0].as_u64(), Some(2));
        assert_eq!(
            sequence["plate_1"]["nozzle_sequence"][0].as_str(),
            Some("2")
        );

        // Verify filament settings files are remapped and cleaned up
        assert!(!entries.contains_key("Metadata/filament_settings_3.config"));
        assert!(entries.contains_key("Metadata/filament_settings_2.config"));
        let fil_set2: serde_json::Value =
            serde_json::from_slice(&entries["Metadata/filament_settings_2.config"]).unwrap();
        assert_eq!(fil_set2["name"].as_str(), Some("Custom third"));
    }
}
