use std::collections::BTreeMap;

use anyhow::{bail, Result};
use regex::{Captures, Regex};

#[derive(Debug, Clone, Default)]
pub struct Remap {
    pub ids: BTreeMap<u32, u32>,
    pub paths: BTreeMap<String, String>,
    pub filament_offset: usize,
}

impl Remap {
    pub fn map_id(&self, id: u32) -> Result<u32> {
        self.ids
            .get(&id)
            .copied()
            .ok_or_else(|| anyhow::anyhow!("missing object id remap for {id}"))
    }

    pub fn map_path_value(&self, path: &str) -> String {
        let normalized = path.trim_start_matches('/');
        self.paths
            .get(normalized)
            .map(|mapped| format!("/{mapped}"))
            .unwrap_or_else(|| path.to_string())
    }
}

pub fn collect_resource_ids(xml: &str) -> Result<Vec<u32>> {
    let re = Regex::new(
        r#"<(?:object|basematerials|colorgroup|texture2d|texture2dgroup|compositematerials|multiproperties)\b[^>]*\bid="(\d+)""#,
    )?;
    let mut ids = Vec::new();
    for captures in re.captures_iter(xml) {
        ids.push(captures[1].parse()?);
    }
    Ok(ids)
}

pub fn rewrite_model_xml(xml: &str, remap: &Remap) -> Result<String> {
    let resource_id_re = Regex::new(
        r#"(<(?:object|basematerials|colorgroup|texture2d|texture2dgroup|compositematerials|multiproperties)\b[^>]*\bid=")(\d+)(")"#,
    )?;
    let object_ref_re = Regex::new(r#"(\bobjectid=")(\d+)(")"#)?;
    let property_ref_re = Regex::new(r#"(\bpid=")(\d+)(")"#)?;
    let path_re = Regex::new(r#"(\bp:path=")([^"]+)(")"#)?;

    let xml = replace_id_attrs(&resource_id_re, xml, remap)?;
    let xml = replace_id_attrs(&object_ref_re, &xml, remap)?;
    let xml = replace_id_attrs(&property_ref_re, &xml, remap)?;
    let xml = path_re
        .replace_all(&xml, |captures: &Captures<'_>| {
            format!(
                "{}{}{}",
                &captures[1],
                remap.map_path_value(&captures[2]),
                &captures[3]
            )
        })
        .into_owned();

    let xml = if remap.filament_offset > 0 {
        let face_property_re = Regex::new(r#"(\bface_property=")(\d+)(")"#)?;
        face_property_re
            .replace_all(&xml, |captures: &Captures<'_>| {
                let val: usize = captures[2].parse().unwrap();
                if val > 0 {
                    format!(
                        "{}{}{}",
                        &captures[1],
                        val + remap.filament_offset,
                        &captures[3]
                    )
                } else {
                    captures[0].to_string()
                }
            })
            .into_owned()
    } else {
        xml
    };

    Ok(xml)
}

pub fn collect_metadata_elements(xml: &str) -> Result<Vec<String>> {
    let re = Regex::new(r#"(?s)<metadata\b[^>]*/>|<metadata\b[^>]*>.*?</metadata>"#)?;
    Ok(re
        .find_iter(xml)
        .map(|matched| matched.as_str().to_string())
        .collect())
}

pub fn rewrite_metadata_path_text(metadata: &str, paths: &BTreeMap<String, String>) -> String {
    let mut rewritten = metadata.to_string();
    for (source, target) in paths {
        rewritten = rewritten.replace(&format!(">/{source}<"), &format!(">/{target}<"));
    }
    rewritten
}

pub fn rewrite_production_uuids(
    xml: &str,
    input_number: usize,
    next_uuid_index: &mut u32,
) -> Result<String> {
    let re = Regex::new(r#"(\bp:UUID=")([^"]+)(")"#)?;
    Ok(re
        .replace_all(xml, |captures: &Captures<'_>| {
            let rewritten = deterministic_production_uuid(input_number, *next_uuid_index);
            *next_uuid_index += 1;
            format!("{}{}{}", &captures[1], rewritten, &captures[3])
        })
        .into_owned())
}

fn deterministic_production_uuid(input_number: usize, uuid_index: u32) -> String {
    format!(
        "{:08x}-{:04x}-4{:03x}-8{:03x}-{:012x}",
        input_number as u32,
        (uuid_index >> 16) as u16,
        (uuid_index & 0x0fff) as u16,
        ((uuid_index >> 4) & 0x0fff) as u16,
        uuid_index as u64
    )
}

pub fn prefix_metadata_name(metadata: &str, prefix: &str) -> Result<String> {
    let re = Regex::new(r#"(<metadata\b[^>]*\bname=")([^"]+)(")"#)?;
    Ok(re
        .replace_all(metadata, |captures: &Captures<'_>| {
            format!(
                "{}{}{}",
                &captures[1],
                prefix_metadata_name_value(&captures[2], prefix),
                &captures[3]
            )
        })
        .into_owned())
}

fn prefix_metadata_name_value(name: &str, prefix: &str) -> String {
    if let Some((namespace, local_name)) = name.split_once(':') {
        format!("{namespace}:{prefix}{local_name}")
    } else {
        format!("{prefix}{name}")
    }
}

pub fn rewrite_bambu_model_settings(xml: &str, remap: &Remap) -> Result<String> {
    let object_id_re = Regex::new(r#"(<object\b[^>]*\bid=")(\d+)(")"#)?;
    let part_id_re = Regex::new(r#"(<part\b[^>]*\bid=")(\d+)(")"#)?;
    let source_object_re =
        Regex::new(r#"(<metadata\b[^>]*\bkey="source_object_id"[^>]*\bvalue=")(\d+)(")"#)?;

    let xml = replace_id_attrs_if_mapped(&object_id_re, xml, remap)?;
    let xml = replace_id_attrs_if_mapped(&part_id_re, &xml, remap)?;
    let xml = replace_id_attrs_if_mapped(&source_object_re, &xml, remap)?;

    let xml = if remap.filament_offset > 0 {
        let extruder_re =
            Regex::new(r#"(<metadata\b[^>]*\bkey="extruder"[^>]*\bvalue=")(\d+)(")"#)?;
        extruder_re
            .replace_all(&xml, |captures: &Captures<'_>| {
                let val: usize = captures[2].parse().unwrap();
                if val > 0 {
                    format!(
                        "{}{}{}",
                        &captures[1],
                        val + remap.filament_offset,
                        &captures[3]
                    )
                } else {
                    captures[0].to_string()
                }
            })
            .into_owned()
    } else {
        xml
    };

    Ok(xml)
}

pub fn config_object_elements(xml: &str) -> Result<Vec<String>> {
    let re = Regex::new(r#"(?s)<object\b[^>]*>.*?</object>"#)?;
    Ok(re
        .find_iter(xml)
        .map(|matched| matched.as_str().to_string())
        .collect())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relationship {
    pub id: String,
    pub target: String,
    pub kind: String,
}

pub fn parse_relationships(xml: &str) -> Result<Vec<Relationship>> {
    let relationship_re = Regex::new(r#"<Relationship\b([^>]*)/?>"#)?;
    let id_re = Regex::new(r#"\bId="([^"]*)"|\bId='([^']*)'"#)?;
    let target_re = Regex::new(r#"\bTarget="([^"]*)"|\bTarget='([^']*)'"#)?;
    let type_re = Regex::new(r#"\bType="([^"]*)"|\bType='([^']*)'"#)?;
    let mut relationships = Vec::new();

    for captures in relationship_re.captures_iter(xml) {
        let attrs = &captures[1];
        let Some(id) = capture_attr(&id_re, attrs) else {
            continue;
        };
        let Some(target) = capture_attr(&target_re, attrs) else {
            continue;
        };
        let Some(kind) = capture_attr(&type_re, attrs) else {
            continue;
        };

        relationships.push(Relationship { id, target, kind });
    }

    Ok(relationships)
}

fn capture_attr(re: &Regex, attrs: &str) -> Option<String> {
    re.captures(attrs)
        .and_then(|captures| captures.get(1).or_else(|| captures.get(2)))
        .map(|value| value.as_str().to_string())
}

fn replace_id_attrs(re: &Regex, xml: &str, remap: &Remap) -> Result<String> {
    let mut out = String::with_capacity(xml.len());
    let mut last = 0;

    for captures in re.captures_iter(xml) {
        let full = captures.get(0).expect("full regex match");
        out.push_str(&xml[last..full.start()]);
        let source_id: u32 = captures[2].parse()?;
        out.push_str(&captures[1]);
        out.push_str(&remap.map_id(source_id)?.to_string());
        out.push_str(&captures[3]);
        last = full.end();
    }

    out.push_str(&xml[last..]);
    Ok(out)
}

fn replace_id_attrs_if_mapped(re: &Regex, xml: &str, remap: &Remap) -> Result<String> {
    let mut out = String::with_capacity(xml.len());
    let mut last = 0;

    for captures in re.captures_iter(xml) {
        let full = captures.get(0).expect("full regex match");
        out.push_str(&xml[last..full.start()]);
        let source_id: u32 = captures[2].parse()?;
        out.push_str(&captures[1]);
        if source_id == 0 {
            out.push('0');
        } else if let Some(mapped) = remap.ids.get(&source_id) {
            out.push_str(&mapped.to_string());
        } else {
            out.push_str(&source_id.to_string());
        }
        out.push_str(&captures[3]);
        last = full.end();
    }

    out.push_str(&xml[last..]);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrites_resource_ids_refs_and_component_paths() {
        let mut remap = Remap::default();
        remap.ids.insert(1, 10);
        remap.ids.insert(2, 11);
        remap.ids.insert(3, 12);
        remap.paths.insert(
            "3D/Objects/object_1.model".to_string(),
            "3D/Objects/input-002-object_1.model".to_string(),
        );

        let xml = r##"<model><resources><basematerials id="3"><base name="red" displaycolor="#ff0000"/></basematerials><object id="2"><components><component p:path="/3D/Objects/object_1.model" objectid="1" /></components><mesh><triangles><triangle v1="0" v2="1" v3="2" pid="3" p1="0"/></triangles></mesh></object></resources><build><item objectid="2" /></build></model>"##;
        let rewritten = rewrite_model_xml(xml, &remap).unwrap();

        assert!(rewritten.contains(r#"<object id="11">"#));
        assert!(rewritten.contains(r#"<basematerials id="12">"#));
        assert!(rewritten.contains(r#"objectid="10""#));
        assert!(rewritten.contains(r#"objectid="11""#));
        assert!(rewritten.contains(r#"pid="12""#));
        assert!(rewritten.contains(r#"p:path="/3D/Objects/input-002-object_1.model""#));
    }

    #[test]
    fn parses_relationships_with_mixed_quotes() {
        let relationships = parse_relationships(
            r#"<Relationships>
 <Relationship Target='/3D/3dmodel.model' Id='rel-1' Type='model'/>
 <Relationship Target="/Auxiliaries/thumb.png" Id="rel-2" Type="thumbnail"/>
</Relationships>"#,
        )
        .unwrap();

        assert_eq!(
            relationships,
            vec![
                Relationship {
                    id: "rel-1".to_string(),
                    target: "/3D/3dmodel.model".to_string(),
                    kind: "model".to_string(),
                },
                Relationship {
                    id: "rel-2".to_string(),
                    target: "/Auxiliaries/thumb.png".to_string(),
                    kind: "thumbnail".to_string(),
                },
            ]
        );
    }

    #[test]
    fn collects_metadata_and_rewrites_path_text() {
        let xml = r#"<model>
 <metadata name="Title">Yoshi</metadata>
 <metadata name="Thumbnail_Middle">/Metadata/plate_1.png</metadata>
 <resources/>
</model>"#;
        let metadata = collect_metadata_elements(xml).unwrap();
        let paths = BTreeMap::from([(
            "Metadata/plate_1.png".to_string(),
            "MergedInputs/input-002/Metadata/plate_1.png".to_string(),
        )]);

        assert_eq!(metadata.len(), 2);
        assert_eq!(
            rewrite_metadata_path_text(&metadata[1], &paths),
            r#"<metadata name="Thumbnail_Middle">/MergedInputs/input-002/Metadata/plate_1.png</metadata>"#
        );
    }

    #[test]
    fn prefixes_metadata_name() {
        let metadata = r#"<metadata name="Title">Yoshi</metadata>"#;
        assert_eq!(
            prefix_metadata_name(metadata, "Input002.").unwrap(),
            r#"<metadata name="Input002.Title">Yoshi</metadata>"#
        );
    }

    #[test]
    fn prefixes_metadata_local_name_when_name_is_namespaced() {
        let metadata = r#"<metadata name="BambuStudio:3mfVersion">1</metadata>"#;
        assert_eq!(
            prefix_metadata_name(metadata, "Input002.").unwrap(),
            r#"<metadata name="BambuStudio:Input002.3mfVersion">1</metadata>"#
        );
    }

    #[test]
    fn prefixes_every_metadata_name_in_a_fragment() {
        let metadata = r#"<metadata name="CreationDate">2026-04-16</metadata>
<metadata name="Description">Luigi</metadata>"#;
        assert_eq!(
            prefix_metadata_name(metadata, "Input002.").unwrap(),
            r#"<metadata name="Input002.CreationDate">2026-04-16</metadata>
<metadata name="Input002.Description">Luigi</metadata>"#
        );
    }

    #[test]
    fn rewrites_production_uuids_with_valid_deterministic_values() {
        let mut next = 1;
        let xml = r#"<object p:UUID="00000002-b1ec-4553-aec9-835e5b724bb4"/><item p:UUID="00000004-b1ec-4553-aec9-835e5b724bb4"/>"#;
        let rewritten = rewrite_production_uuids(xml, 2, &mut next).unwrap();

        assert!(rewritten.contains(r#"p:UUID="00000002-0000-4001-8000-000000000001""#));
        assert!(rewritten.contains(r#"p:UUID="00000002-0000-4002-8000-000000000002""#));
        assert_eq!(next, 3);
    }

    #[test]
    fn rewrites_bambu_model_settings_ids() {
        let mut remap = Remap::default();
        remap.ids.insert(1, 10);
        remap.ids.insert(2, 11);

        let xml = r#"<config>
  <object id="2">
    <part id="1">
      <metadata key="source_object_id" value="1"/>
      <metadata key="source_volume_id" value="0"/>
    </part>
  </object>
</config>"#;
        let rewritten = rewrite_bambu_model_settings(xml, &remap).unwrap();

        assert!(rewritten.contains(r#"<object id="11">"#));
        assert!(rewritten.contains(r#"<part id="10">"#));
        assert!(rewritten.contains(r#"key="source_object_id" value="10""#));
        assert!(rewritten.contains(r#"key="source_volume_id" value="0""#));
        assert_eq!(config_object_elements(&rewritten).unwrap().len(), 1);
    }

    #[test]
    fn rewrites_bambu_plate_object_and_identify_ids() {
        let mut remap = Remap::default();
        remap.ids.insert(2, 20);

        let xml = r#"<plate>
    <metadata key="plater_id" value="1"/>
    <metadata key="filament_maps" value="1"/>
    <metadata key="filament_volume_maps" value="0"/>
    <model_instance>
      <metadata key="object_id" value="2"/>
      <metadata key="identify_id" value="743"/>
    </model_instance>
  </plate>"#;
        let rewritten = rewrite_bambu_plate_element(xml, &remap, 6, 2048, 6, 0).unwrap();

        assert!(rewritten.contains(r#"key="plater_id" value="7""#));
        assert!(rewritten.contains(r#"key="filament_maps" value="0 0 0 0 0 0 1""#));
        assert!(rewritten.contains(r#"key="object_id" value="20""#));
        assert!(rewritten.contains(r#"key="identify_id" value="2791""#));
    }
}

pub fn split_model(xml: &str) -> Result<ModelSections> {
    let resources_start = xml
        .find("<resources>")
        .ok_or_else(|| anyhow::anyhow!("model XML is missing <resources>"))?;
    let resources_end_start = xml
        .find("</resources>")
        .ok_or_else(|| anyhow::anyhow!("model XML is missing </resources>"))?;
    let resources_end = resources_end_start + "</resources>".len();

    let build_start = xml
        .find("<build")
        .ok_or_else(|| anyhow::anyhow!("model XML is missing <build>"))?;
    let build_open_end = xml[build_start..]
        .find('>')
        .map(|offset| build_start + offset + 1)
        .ok_or_else(|| anyhow::anyhow!("model XML has malformed <build>"))?;
    let build_end_start = xml
        .find("</build>")
        .ok_or_else(|| anyhow::anyhow!("model XML is missing </build>"))?;
    let build_end = build_end_start + "</build>".len();

    if resources_start >= resources_end || resources_end > build_start || build_start >= build_end {
        bail!("model XML has unsupported resource/build ordering");
    }

    Ok(ModelSections {
        pre_resources: xml[..resources_start].to_string(),
        resources_inner: xml[resources_start + "<resources>".len()..resources_end_start]
            .to_string(),
        build_open: xml[build_start..build_open_end].to_string(),
        build_inner: xml[build_open_end..build_end_start].to_string(),
        post_build: xml[build_end..].to_string(),
    })
}

#[derive(Debug, Clone)]
pub struct ModelSections {
    pub pre_resources: String,
    pub resources_inner: String,
    pub build_open: String,
    pub build_inner: String,
    pub post_build: String,
}

pub fn config_plate_elements(xml: &str) -> Result<Vec<String>> {
    let re = Regex::new(r#"(?s)<plate\b[^>]*>.*?</plate>"#)?;
    Ok(re
        .find_iter(xml)
        .map(|matched| matched.as_str().to_string())
        .collect())
}

pub fn config_assemble_item_elements(xml: &str) -> Result<Vec<String>> {
    let re = Regex::new(r#"<assemble_item\b[^>]*/>"#)?;
    Ok(re
        .find_iter(xml)
        .map(|matched| matched.as_str().to_string())
        .collect())
}

pub fn rewrite_bambu_plate_element(
    xml: &str,
    remap: &Remap,
    plate_offset: usize,
    identify_id_offset: u32,
    n_before: usize,
    n_after: usize,
) -> Result<String> {
    // 1. Shift plater_id
    let plater_id_re = Regex::new(r#"(<metadata\b[^>]*\bkey="plater_id"[^>]*\bvalue=")(\d+)(")"#)?;
    let xml = plater_id_re
        .replace(xml, |captures: &Captures<'_>| {
            let val: usize = captures[2].parse().unwrap();
            format!("{}{}{}", &captures[1], val + plate_offset, &captures[3])
        })
        .into_owned();

    // 2. Rewrite thumbnail and other plate files
    let file_keys = [
        "thumbnail_file",
        "thumbnail_no_light_file",
        "top_file",
        "pick_file",
    ];
    let mut rewritten = xml;
    for key in file_keys {
        let file_re = Regex::new(&format!(
            r#"(<metadata\b[^>]*\bkey="{}"[^>]*\bvalue=")(Metadata/)(plate_|plate_no_light_|top_|pick_)(\d+)([^"]*)(")"#,
            key
        ))?;
        rewritten = file_re
            .replace_all(&rewritten, |captures: &Captures<'_>| {
                let val: usize = captures[4].parse().unwrap();
                format!(
                    "{}{}{}{}{}{}",
                    &captures[1],
                    &captures[2],
                    &captures[3],
                    val + plate_offset,
                    &captures[5],
                    &captures[6]
                )
            })
            .into_owned();
    }

    // 3. Rewrite filament_maps and filament_volume_maps
    let is_later_input = plate_offset > 0;
    let maps_re = Regex::new(r#"(<metadata\b[^>]*\bkey="filament_maps"[^>]*\bvalue=")([^"]*)(")"#)?;
    rewritten = maps_re
        .replace_all(&rewritten, |captures: &Captures<'_>| {
            let val = &captures[2];
            let parts: Vec<&str> = val.split_whitespace().collect();
            let new_val = if is_later_input {
                let mut new_parts = vec!["0".to_string(); n_before];
                new_parts.extend(parts.iter().map(|s| s.to_string()));
                new_parts.extend(vec!["0".to_string(); n_after]);
                new_parts.join(" ")
            } else {
                let mut new_parts = parts.iter().map(|s| s.to_string()).collect::<Vec<_>>();
                new_parts.extend(vec!["0".to_string(); n_after]);
                new_parts.join(" ")
            };
            format!("{}{}{}", &captures[1], new_val, &captures[3])
        })
        .into_owned();

    let vol_maps_re =
        Regex::new(r#"(<metadata\b[^>]*\bkey="filament_volume_maps"[^>]*\bvalue=")([^"]*)(")"#)?;
    rewritten = vol_maps_re
        .replace_all(&rewritten, |captures: &Captures<'_>| {
            let val = &captures[2];
            let parts: Vec<&str> = val.split_whitespace().collect();
            let new_val = if is_later_input {
                let mut new_parts = vec!["0".to_string(); n_before];
                new_parts.extend(parts.iter().map(|s| s.to_string()));
                new_parts.extend(vec!["0".to_string(); n_after]);
                new_parts.join(" ")
            } else {
                let mut new_parts = parts.iter().map(|s| s.to_string()).collect::<Vec<_>>();
                new_parts.extend(vec!["0".to_string(); n_after]);
                new_parts.join(" ")
            };
            format!("{}{}{}", &captures[1], new_val, &captures[3])
        })
        .into_owned();

    if identify_id_offset > 0 {
        let identify_id_re =
            Regex::new(r#"(<metadata\b[^>]*\bkey="identify_id"[^>]*\bvalue=")(\d+)(")"#)?;
        rewritten = identify_id_re
            .replace_all(&rewritten, |captures: &Captures<'_>| {
                let val: u32 = captures[2].parse().unwrap();
                format!(
                    "{}{}{}",
                    &captures[1],
                    val + identify_id_offset,
                    &captures[3]
                )
            })
            .into_owned();
    }

    // 4. Rewrite model_instance object_id
    let object_id_re = Regex::new(r#"(<metadata\b[^>]*\bkey="object_id"[^>]*\bvalue=")(\d+)(")"#)?;
    let mut out = String::with_capacity(rewritten.len());
    let mut last = 0;
    for captures in object_id_re.captures_iter(&rewritten) {
        let full = captures.get(0).unwrap();
        out.push_str(&rewritten[last..full.start()]);
        let source_id: u32 = captures[2].parse()?;
        out.push_str(&captures[1]);
        if let Some(mapped) = remap.ids.get(&source_id) {
            out.push_str(&mapped.to_string());
        } else {
            out.push_str(&source_id.to_string());
        }
        out.push_str(&captures[3]);
        last = full.end();
    }
    out.push_str(&rewritten[last..]);

    Ok(out)
}

pub fn rewrite_bambu_assemble_item_element(xml: &str, remap: &Remap) -> Result<String> {
    let object_id_re = Regex::new(r#"(<assemble_item\b[^>]*\bobject_id=")(\d+)(")"#)?;
    let mut out = String::with_capacity(xml.len());
    let mut last = 0;
    for captures in object_id_re.captures_iter(xml) {
        let full = captures.get(0).unwrap();
        out.push_str(&xml[last..full.start()]);
        let source_id: u32 = captures[2].parse()?;
        out.push_str(&captures[1]);
        if let Some(mapped) = remap.ids.get(&source_id) {
            out.push_str(&mapped.to_string());
        } else {
            out.push_str(&source_id.to_string());
        }
        out.push_str(&captures[3]);
        last = full.end();
    }
    out.push_str(&xml[last..]);
    Ok(out)
}
