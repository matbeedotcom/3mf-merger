use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use tempfile::NamedTempFile;
use thiserror::Error;

use crate::package::{write_package, Package};
use crate::rewrite::{
    collect_metadata_elements, collect_resource_ids, config_object_elements, parse_relationships,
    prefix_metadata_name, rewrite_bambu_model_settings, rewrite_metadata_path_text,
    rewrite_model_xml, rewrite_production_uuids, split_model, Relationship, Remap,
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

pub fn merge_files(inputs: &[PathBuf], output: &Path, force: bool) -> Result<()> {
    if inputs.len() < 2 {
        return Err(MergeError::TooFewInputs.into());
    }
    if output.exists() && !force {
        return Err(MergeError::OutputExists(output.to_path_buf()).into());
    }

    let merged = merge_packages(inputs)?;
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

fn merge_packages(inputs: &[PathBuf]) -> Result<BTreeMap<String, Vec<u8>>> {
    let mut loaded = Vec::with_capacity(inputs.len());
    for input in inputs {
        loaded.push(Package::read(input)?);
    }

    let mut next_id = 1;
    let mut merged_resources = String::new();
    let mut merged_build = String::new();
    let mut appended_metadata = String::new();
    let mut merged_model_settings = String::new();
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
                rewritten =
                    rewrite_production_uuids(&rewritten, index + 1, &mut next_uuid_index)
                        .with_context(|| {
                        format!("failed to rewrite production UUIDs for {source_path}")
                    })?;
            }
            output_entries.insert(mapped_path.clone(), rewritten.into_bytes());
            model_rel_targets.push(format!("/{mapped_path}"));
        }

        let mut rewritten_model = rewrite_model_xml(&source_model, &remap)
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

        let auxiliary_paths =
            copy_auxiliary_entries(index, package, &mut output_entries, &mut next_plate_index)?;
        if index > 0 {
            for metadata in collect_metadata_elements(&rewritten_model)? {
                let metadata = rewrite_metadata_path_text(&metadata, &auxiliary_paths);
                let metadata = prefix_metadata_name(&metadata, &format!("Input{:03}.", index + 1))?;
                appended_metadata.push_str(" ");
                appended_metadata.push_str(&metadata);
                appended_metadata.push('\n');
            }
        }
        if let Some(settings) = package.entries.get(MODEL_SETTINGS) {
            let settings = String::from_utf8(settings.clone())
                .context("model_settings.config is not UTF-8")?;
            let rewritten_settings = rewrite_bambu_model_settings(&settings, &remap)?;
            for object in config_object_elements(&rewritten_settings)? {
                merged_model_settings.push_str("  ");
                merged_model_settings.push_str(&object);
                merged_model_settings.push('\n');
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
    if !merged_model_settings.is_empty() {
        output_entries.insert(
            MODEL_SETTINGS.to_string(),
            model_settings_xml(&merged_model_settings).into_bytes(),
        );
    }

    Ok(output_entries)
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
        copied_paths.insert(path.clone(), target.clone());
        output_entries.insert(target, bytes.clone());
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

fn model_settings_xml(objects: &str) -> String {
    format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<config>\n{objects}</config>\n")
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
        let err = merge_files(&[], Path::new("out.3mf"), false).unwrap_err();
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
}
