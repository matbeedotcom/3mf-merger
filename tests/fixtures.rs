use std::collections::BTreeSet;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;

use tempfile::tempdir;
use zip::ZipArchive;

#[test]
fn merges_luigi_and_yoshi_fixture_package() {
    let luigi = PathBuf::from("Luigi.3mf");
    let yoshi = PathBuf::from("Yoshi.3mf");
    if !luigi.exists() || !yoshi.exists() {
        return;
    }

    let tempdir = tempdir().unwrap();
    let output = tempdir.path().join("merged.3mf");
    three_mf_merger::merge_files(&[luigi, yoshi], &output, false).unwrap();

    let file = File::open(&output).unwrap();
    let mut archive = ZipArchive::new(file).unwrap();

    assert!(archive.by_name("3D/3dmodel.model").is_ok());
    assert!(archive
        .by_name("3D/Objects/input-002-object_79.model")
        .is_ok());
    assert!(archive
        .by_name("MergedInputs/input-002/Auxiliaries/.thumbnails/thumbnail_3mf.png")
        .is_ok());
    assert!(archive.by_name("Metadata/plate_8.png").is_ok());
    assert!(archive.by_name("Metadata/top_8.png").is_ok());
    assert!(archive.by_name("Metadata/pick_8.png").is_ok());
    assert!(archive.by_name("Metadata/plate_9.json").is_ok());

    let mut model = String::new();
    archive
        .by_name("3D/3dmodel.model")
        .unwrap()
        .read_to_string(&mut model)
        .unwrap();
    assert!(model.contains(r#"p:path="/3D/Objects/input-002-object_79.model""#));
    assert!(model.contains(r#"<object id="91""#));
    assert!(model.contains("耀西Yoshi 来自马力欧兄弟 关节可动人偶，无需AMS"));
    assert!(model.contains(
        r#"<metadata name="Input002.Thumbnail_Middle">/Metadata/plate_8.png</metadata>"#
    ));
    assert!(
        model.contains(r#"<metadata name="Input002.DesignModelId">USd3af4a4d3f5cb8</metadata>"#)
    );
    assert_eq!(model.matches("<item objectid=").count(), 87);

    let mut model_settings = String::new();
    archive
        .by_name("Metadata/model_settings.config")
        .unwrap()
        .read_to_string(&mut model_settings)
        .unwrap();
    assert!(model_settings.contains("Yoshi插销-鼻子.stl"));
    assert!(model_settings.contains("hat pin"));
    assert_eq!(plate_entries_matching(&archive_names(&output), ".png"), 13);
    assert_eq!(plate_entries_matching(&archive_names(&output), ".json"), 8);

    let mut root_rels = String::new();
    archive
        .by_name("_rels/.rels")
        .unwrap()
        .read_to_string(&mut root_rels)
        .unwrap();
    assert!(root_rels.contains("/Auxiliaries/.thumbnails/thumbnail_3mf.png"));
    assert!(root_rels.contains("/MergedInputs/input-002/Auxiliaries/.thumbnails/thumbnail_3mf.png"));

    let mut model_rels = String::new();
    archive
        .by_name("3D/_rels/3dmodel.model.rels")
        .unwrap()
        .read_to_string(&mut model_rels)
        .unwrap();
    assert_eq!(model_rels.matches("Id=\"rel-").count(), 87);
    assert!(model_rels.contains("Id=\"rel-87\""));
}

fn plate_entries_matching(entries: &BTreeSet<String>, suffix: &str) -> usize {
    entries
        .iter()
        .filter(|entry| {
            let Some(name) = entry.strip_prefix("Metadata/plate_") else {
                return false;
            };
            name.chars().next().is_some_and(|ch| ch.is_ascii_digit()) && entry.ends_with(suffix)
        })
        .count()
}

#[test]
fn fixture_input_entries_are_accounted_for_in_output() {
    let luigi = PathBuf::from("Luigi.3mf");
    let yoshi = PathBuf::from("Yoshi.3mf");
    if !luigi.exists() || !yoshi.exists() {
        return;
    }

    let tempdir = tempdir().unwrap();
    let output = tempdir.path().join("merged.3mf");
    three_mf_merger::merge_files(&[luigi.clone(), yoshi.clone()], &output, false).unwrap();

    let luigi_entries = archive_names(&luigi);
    let yoshi_entries = archive_names(&yoshi);
    let output_entries = archive_names(&output);

    for entry in &luigi_entries {
        assert!(
            accounted_first_input(entry, &output_entries),
            "missing Luigi entry representation: {entry}"
        );
    }

    for entry in &yoshi_entries {
        assert!(
            accounted_later_input(entry, 2, 7, &output_entries),
            "missing Yoshi entry representation: {entry}"
        );
    }
}

fn archive_names(path: &PathBuf) -> BTreeSet<String> {
    let file = File::open(path).unwrap();
    let mut archive = ZipArchive::new(file).unwrap();
    let mut names = BTreeSet::new();
    for index in 0..archive.len() {
        let file = archive.by_index(index).unwrap();
        if !file.is_dir() {
            names.insert(file.name().to_string());
        }
    }
    names
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
