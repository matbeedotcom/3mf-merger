use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use tempfile::tempdir;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

#[test]
fn remaps_colliding_material_object_ids_and_object_paths() {
    let tempdir = tempdir().unwrap();
    let first = tempdir.path().join("first.3mf");
    let second = tempdir.path().join("second.3mf");
    let output = tempdir.path().join("merged.3mf");

    write_synthetic_3mf(&first, "#ff0000").unwrap();
    write_synthetic_3mf(&second, "#00ff00").unwrap();

    three_mf_merger::merge_files(
        &[first, second],
        &output,
        false,
        false,
        false,
        false,
        false,
        false,
        false,
        false,
    )
    .unwrap();

    let file = File::open(&output).unwrap();
    let mut archive = ZipArchive::new(file).unwrap();

    let mut top_model = String::new();
    archive
        .by_name("3D/3dmodel.model")
        .unwrap()
        .read_to_string(&mut top_model)
        .unwrap();
    assert!(top_model.contains(r#"<object id="3""#));
    assert!(top_model.contains(r#"<object id="6""#));
    assert!(top_model.contains(r#"objectid="5""#));
    assert!(top_model.contains(r#"p:path="/3D/Objects/input-002-object_1.model""#));

    let mut second_object = String::new();
    archive
        .by_name("3D/Objects/input-002-object_1.model")
        .unwrap()
        .read_to_string(&mut second_object)
        .unwrap();
    assert!(second_object.contains(r#"<basematerials id="4">"#));
    assert!(second_object.contains(r#"<object id="5""#));
    assert!(second_object.contains(r#"pid="4""#));
    assert!(second_object.contains("#00ff00"));
}

#[test]
fn repeated_merge_is_byte_deterministic_for_same_inputs() {
    let tempdir = tempdir().unwrap();
    let first = tempdir.path().join("first.3mf");
    let second = tempdir.path().join("second.3mf");
    let output_a = tempdir.path().join("merged-a.3mf");
    let output_b = tempdir.path().join("merged-b.3mf");

    write_synthetic_3mf(&first, "#ff0000").unwrap();
    write_synthetic_3mf(&second, "#00ff00").unwrap();

    three_mf_merger::merge_files(
        &[first.clone(), second.clone()],
        &output_a,
        false,
        false,
        false,
        false,
        false,
        false,
        false,
        false,
    )
    .unwrap();
    three_mf_merger::merge_files(
        &[first, second],
        &output_b,
        false,
        false,
        false,
        false,
        false,
        false,
        false,
        false,
    )
    .unwrap();

    let bytes_a = std::fs::read(output_a).unwrap();
    let bytes_b = std::fs::read(output_b).unwrap();
    assert_eq!(bytes_a, bytes_b);
}

fn write_synthetic_3mf(path: &Path, color: &str) -> zip::result::ZipResult<()> {
    let file = File::create(path)?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(0o644);

    write_entry(
        &mut zip,
        options,
        "[Content_Types].xml",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
 <Default Extension="model" ContentType="application/vnd.ms-package.3dmanufacturing-3dmodel+xml"/>
 <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
</Types>
"#,
    )?;
    write_entry(
        &mut zip,
        options,
        "_rels/.rels",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
 <Relationship Target="/3D/3dmodel.model" Id="rel-1" Type="http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel"/>
</Relationships>
"#,
    )?;
    write_entry(
        &mut zip,
        options,
        "3D/_rels/3dmodel.model.rels",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
 <Relationship Target="/3D/Objects/object_1.model" Id="rel-1" Type="http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel"/>
</Relationships>
"#,
    )?;
    write_entry(
        &mut zip,
        options,
        "3D/3dmodel.model",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<model xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02" xmlns:p="http://schemas.microsoft.com/3dmanufacturing/production/2015/06" unit="millimeter" requiredextensions="p">
 <resources>
  <object id="3" type="model">
   <components>
    <component p:path="/3D/Objects/object_1.model" objectid="2"/>
   </components>
  </object>
 </resources>
 <build>
  <item objectid="3" printable="1"/>
 </build>
</model>
"#,
    )?;
    write_entry(
        &mut zip,
        options,
        "3D/Objects/object_1.model",
        &format!(
            r##"<?xml version="1.0" encoding="UTF-8"?>
<model xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02" unit="millimeter">
 <resources>
  <basematerials id="1">
   <base name="material" displaycolor="{color}"/>
  </basematerials>
  <object id="2" type="model">
   <mesh>
    <vertices>
     <vertex x="0" y="0" z="0"/>
     <vertex x="1" y="0" z="0"/>
     <vertex x="0" y="1" z="0"/>
    </vertices>
    <triangles>
     <triangle v1="0" v2="1" v3="2" pid="1" p1="0"/>
    </triangles>
   </mesh>
  </object>
 </resources>
 <build/>
</model>
"##
        ),
    )?;

    zip.finish()?;
    Ok(())
}

fn write_entry<W: Write + std::io::Seek>(
    zip: &mut ZipWriter<W>,
    options: SimpleFileOptions,
    name: &str,
    contents: &str,
) -> zip::result::ZipResult<()> {
    zip.start_file(name, options)?;
    zip.write_all(contents.as_bytes())?;
    Ok(())
}
