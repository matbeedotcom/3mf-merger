use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use lib3mf::{Model, ParserConfig, SpecConformance};
use regex::Regex;
use tempfile::tempdir;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

const LUIGI: &str = "Luigi.3mf";
const YOSHI: &str = "Yoshi.3mf";

#[test]
fn compare_package_counts_to_lib3mf_visible_counts() {
    if !Path::new(LUIGI).exists() || !Path::new(YOSHI).exists() {
        return;
    }

    let merged = merge_fixture();
    let package = PackageCounts::read(&merged);

    let raw_result = parse_lenient(&merged);
    assert!(
        raw_result.is_err(),
        "raw Bambu-flavored output unexpectedly parsed without sanitization"
    );
    let raw_error = raw_result.unwrap_err().to_string();
    assert!(
        raw_error.contains("face_property") || raw_error.contains("paint_supports"),
        "expected lib3mf to reject Bambu triangle painting attributes, got: {raw_error}"
    );

    let vendor_sanitized = sanitize_bambu_triangle_attrs(&merged);
    let vendor_sanitized_error = parse_lenient(&vendor_sanitized).unwrap_err().to_string();
    assert!(
        vendor_sanitized_error.contains("negative determinant"),
        "expected lib3mf to reject preserved Bambu mirror transforms, got: {vendor_sanitized_error}"
    );

    let lib3mf_projection = sanitize_for_lib3mf_standard_projection(&merged);
    let lib3mf = parse_lenient(&lib3mf_projection).unwrap();

    assert_eq!(package.build_items, 87);
    assert_eq!(lib3mf.build.items.len(), 87);

    assert_eq!(package.top_level_objects, 87);
    assert_eq!(lib3mf.resources.objects.len(), 87);

    assert_eq!(package.object_parts, 87);
    assert_eq!(package.yoshi_promoted_object_parts, 36);

    assert_eq!(package.plate_png, 13);
    assert_eq!(package.plate_json, 8);
    assert_eq!(package.top_png, 13);
    assert_eq!(package.pick_png, 13);
    assert_eq!(package.plate_no_light_png, 13);

    // lib3mf exposes standard 3MF core/material data. Bambu plates are slicer
    // metadata files, not standard 3MF model resources.
    assert_eq!(lib3mf_visible_plate_count(&lib3mf), 0);

    assert_eq!(package.face_property_attrs, 22_731);
    assert_eq!(package.paint_supports_attrs, 616);
    assert_eq!(package.build_item_transforms, 87);
    assert_eq!(package.standard_pid_refs, 0);

    // The colors/paint/support painting in these fixtures are Bambu-specific
    // triangle attributes, not standard Materials Extension resources.
    assert_eq!(lib3mf.resources.base_material_groups.len(), 0);
    assert_eq!(lib3mf.resources.color_groups.len(), 0);
    assert_eq!(lib3mf.resources.texture2d_resources.len(), 0);
    assert_eq!(lib3mf.resources.texture2d_groups.len(), 0);
    assert_eq!(lib3mf.resources.composite_materials.len(), 0);
    assert_eq!(lib3mf.resources.multi_properties.len(), 0);

    assert!(lib3mf
        .metadata
        .iter()
        .any(|entry| entry.value.contains("路易吉Luigi")));
    assert!(lib3mf
        .metadata
        .iter()
        .any(|entry| entry.value.contains("耀西Yoshi")));
}

#[test]
#[ignore = "diagnostic: strict lib3mf parsing exposes slicer/OPC conformance issues"]
fn lib3mf_strict_parser_diagnostic() {
    if !Path::new(LUIGI).exists() || !Path::new(YOSHI).exists() {
        return;
    }

    let output = merge_fixture();
    for path in [PathBuf::from(LUIGI), PathBuf::from(YOSHI), output] {
        let result = Model::from_reader(File::open(&path).unwrap());
        assert!(
            result.is_ok(),
            "{} failed strict parse: {result:?}",
            path.display()
        );
    }
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
        false, // dedupe_filaments
    )
    .unwrap();
    output
}

fn parse_lenient(path: impl AsRef<Path>) -> lib3mf::Result<Model> {
    let config =
        ParserConfig::with_all_extensions().with_spec_conformance(SpecConformance::Lenient);
    Model::from_reader_with_config(File::open(path.as_ref()).unwrap(), config)
}

fn lib3mf_visible_plate_count(_model: &Model) -> usize {
    0
}

struct PackageCounts {
    build_items: usize,
    top_level_objects: usize,
    object_parts: usize,
    yoshi_promoted_object_parts: usize,
    plate_png: usize,
    plate_json: usize,
    top_png: usize,
    pick_png: usize,
    plate_no_light_png: usize,
    face_property_attrs: usize,
    paint_supports_attrs: usize,
    build_item_transforms: usize,
    standard_pid_refs: usize,
}

impl PackageCounts {
    fn read(path: &Path) -> Self {
        let mut archive = ZipArchive::new(File::open(path).unwrap()).unwrap();
        let mut names = Vec::new();
        for index in 0..archive.len() {
            let file = archive.by_index(index).unwrap();
            if !file.is_dir() {
                names.push(file.name().to_string());
            }
        }

        let model = read_zip_text(path, "3D/3dmodel.model");
        let mut all_model_xml = model.clone();
        for name in names
            .iter()
            .filter(|name| name.starts_with("3D/Objects/") && name.ends_with(".model"))
        {
            all_model_xml.push_str(&read_zip_text(path, name));
        }

        Self {
            build_items: model.matches("<item objectid=").count(),
            top_level_objects: Regex::new(r#"<object\b[^>]*\bid=""#)
                .unwrap()
                .find_iter(&model)
                .count(),
            object_parts: names
                .iter()
                .filter(|name| name.starts_with("3D/Objects/") && name.ends_with(".model"))
                .count(),
            yoshi_promoted_object_parts: names
                .iter()
                .filter(|name| name.starts_with("3D/Objects/input-002-"))
                .count(),
            plate_png: numbered_metadata_count(&names, "plate_", ".png"),
            plate_json: numbered_metadata_count(&names, "plate_", ".json"),
            top_png: numbered_metadata_count(&names, "top_", ".png"),
            pick_png: numbered_metadata_count(&names, "pick_", ".png"),
            plate_no_light_png: numbered_metadata_count(&names, "plate_no_light_", ".png"),
            face_property_attrs: all_model_xml.matches("face_property=").count(),
            paint_supports_attrs: all_model_xml.matches("paint_supports=").count(),
            build_item_transforms: Regex::new(r#"<item\b[^>]*\btransform=""#)
                .unwrap()
                .find_iter(&model)
                .count(),
            standard_pid_refs: Regex::new(r#"\bpid=""#)
                .unwrap()
                .find_iter(&all_model_xml)
                .count(),
        }
    }
}

fn numbered_metadata_count(names: &[String], prefix: &str, suffix: &str) -> usize {
    names
        .iter()
        .filter(|name| {
            let Some(rest) = name.strip_prefix(&format!("Metadata/{prefix}")) else {
                return false;
            };
            rest.chars().next().is_some_and(|ch| ch.is_ascii_digit()) && name.ends_with(suffix)
        })
        .count()
}

fn read_zip_text(path: &Path, name: &str) -> String {
    let mut archive = ZipArchive::new(File::open(path).unwrap()).unwrap();
    let mut text = String::new();
    archive
        .by_name(name)
        .unwrap()
        .read_to_string(&mut text)
        .unwrap();
    text
}

fn sanitize_bambu_triangle_attrs(path: &Path) -> PathBuf {
    sanitize_model_xml(path, false)
}

fn sanitize_for_lib3mf_standard_projection(path: &Path) -> PathBuf {
    sanitize_model_xml(path, true)
}

fn sanitize_model_xml(path: &Path, strip_build_transforms: bool) -> PathBuf {
    let dir = tempdir().unwrap().keep();
    let sanitized = dir.join("sanitized.3mf");
    let mut input = ZipArchive::new(File::open(path).unwrap()).unwrap();
    let mut output = ZipWriter::new(File::create(&sanitized).unwrap());
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(0o644);
    let bambu_triangle_attr_re =
        Regex::new(r#"\s+(?:face_property|paint_supports)="[^"]*""#).unwrap();
    let transform_attr_re = Regex::new(r#"\s+transform="[^"]*""#).unwrap();

    for index in 0..input.len() {
        let mut file = input.by_index(index).unwrap();
        if file.is_dir() {
            continue;
        }

        let name = file.name().to_string();
        output.start_file(&name, options).unwrap();
        if name.ends_with(".model") {
            let mut text = String::new();
            file.read_to_string(&mut text).unwrap();
            let text = bambu_triangle_attr_re.replace_all(&text, "");
            let text = if strip_build_transforms {
                transform_attr_re.replace_all(&text, "").into_owned()
            } else {
                text.into_owned()
            };
            output.write_all(text.as_bytes()).unwrap();
        } else {
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes).unwrap();
            output.write_all(&bytes).unwrap();
        }
    }

    output.finish().unwrap();
    sanitized
}
