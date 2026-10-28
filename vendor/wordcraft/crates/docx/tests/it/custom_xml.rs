//! Original synthetic custom XML stores, written from the public OOXML package contract.
use std::collections::BTreeMap;
use std::io::{Cursor, Read, Write};
use wordcraft_doc::{Path, StoryRef};

const REL_NS: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const RT: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/";
const DATA: &str =
    "<?xml version=\"1.0\"?><c:customer xmlns:c=\"urn:original:customer\"><c:name>Atlas Studios</c:name><c:ref>CASE-17</c:ref></c:customer>";
const PROPS: &str = r#"<ds:datastoreItem xmlns:ds="http://schemas.openxmlformats.org/officeDocument/2006/customXml" ds:itemID="{22222222-2222-2222-2222-222222222222}"><ds:schemaRefs><ds:schemaRef ds:uri="urn:original:customer"/></ds:schemaRefs></ds:datastoreItem>"#;

fn rels(rows: &[(&str, &str, &str, bool)]) -> String {
    let mut out = format!(r#"<Relationships xmlns="{REL_NS}">"#);
    for (id, kind, target, external) in rows {
        let mode = if *external { r#" TargetMode="External""# } else { "" };
        out.push_str(&format!(r#"<Relationship Id="{id}" Type="{RT}{kind}" Target="{target}"{mode}/>"#));
    }
    out + "</Relationships>"
}

fn fixture() -> BTreeMap<String, Vec<u8>> {
    let doc = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Independent edit.</w:t></w:r></w:p><w:p><w:sdt><w:sdtPr><w:id w:val="42"/><w:dataBinding w:xpath="/c:customer/c:name" w:prefixMappings="xmlns:c='urn:original:customer'" w:storeItemID="{22222222-2222-2222-2222-222222222222}"/><w:text/></w:sdtPr><w:sdtContent><w:r><w:t>Atlas Studios</w:t></w:r></w:sdtContent></w:sdt></w:p></w:body></w:document>"#;
    let types = r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="xml" ContentType="application/xml"/><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/customXml/itemProps1.xml" ContentType="application/vnd.openxmlformats-officedocument.customXmlProperties+xml"/></Types>"#;
    [
        ("[Content_Types].xml", types.to_string()),
        ("_rels/.rels", rels(&[("office", "officeDocument", "word/document.xml", false)])),
        ("word/document.xml", doc.to_string()),
        ("word/_rels/document.xml.rels", rels(&[("data", "customXml", "../customXml/item1.xml", false)])),
        ("customXml/item1.xml", DATA.to_string()),
        ("customXml/itemProps1.xml", PROPS.to_string()),
        ("customXml/_rels/item1.xml.rels", rels(&[("properties", "customXmlProps", "itemProps1.xml", false)])),
    ]
    .into_iter()
    .map(|(p, s)| (p.into(), s.into_bytes()))
    .collect()
}

fn zip(parts: &BTreeMap<String, Vec<u8>>) -> Vec<u8> {
    let mut z = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in parts {
        z.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
        z.write_all(bytes).unwrap();
    }
    z.finish().unwrap().into_inner()
}

fn unzip(bytes: &[u8]) -> BTreeMap<String, Vec<u8>> {
    let mut z = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut parts = BTreeMap::new();
    for i in 0..z.len() {
        let mut f = z.by_index(i).unwrap();
        let name = f.name().to_string();
        let mut bytes = Vec::new();
        f.read_to_end(&mut bytes).unwrap();
        assert!(parts.insert(name, bytes).is_none(), "duplicate ZIP entry");
    }
    parts
}

fn text(parts: &BTreeMap<String, Vec<u8>>, name: &str) -> String {
    String::from_utf8(parts[name].clone()).unwrap()
}

fn edited_cycles(input: &BTreeMap<String, Vec<u8>>, check: impl Fn(&BTreeMap<String, Vec<u8>>)) {
    let mut doc = wordcraft_docx::read(&zip(input)).unwrap();
    for _ in 0..2 {
        doc.para_mut(StoryRef::Body, &Path::top(0)).unwrap().insert_text(0, "Unrelated edit. ", &Default::default()).unwrap();
        let bytes = wordcraft_docx::write(&doc).unwrap();
        check(&unzip(&bytes));
        doc = wordcraft_docx::read(&bytes).unwrap();
    }
}

#[test]
fn bound_store_bytes_properties_and_relationships_survive_two_edited_saves() {
    edited_cycles(&fixture(), |p| {
        let document = text(p, "word/document.xml");
        assert!(document.contains("<w:dataBinding"), "the control keeps its store binding");
        assert!(document.contains(r#"w:storeItemID="{22222222-2222-2222-2222-222222222222}""#));
        assert!(document.contains(r#"w:xpath="/c:customer/c:name""#));
        assert_eq!(p.get("customXml/item1.xml").map(Vec::as_slice), Some(DATA.as_bytes()));
        assert_eq!(p.get("customXml/itemProps1.xml").map(Vec::as_slice), Some(PROPS.as_bytes()));
        assert!(text(p, "word/_rels/document.xml.rels").contains(r#"Target="../customXml/item1.xml""#));
        assert!(text(p, "customXml/_rels/item1.xml.rels").contains(r#"Id="properties""#));
        assert!(text(p, "customXml/_rels/item1.xml.rels").contains(r#"Target="itemProps1.xml""#));
        assert!(text(p, "[Content_Types].xml").contains("application/vnd.openxmlformats-officedocument.customXmlProperties+xml"));
    });
}

#[test]
fn package_and_document_roots_share_parts_and_keep_multiple_stores() {
    let mut p = fixture();
    p.insert(
        "_rels/.rels".into(),
        rels(&[("office", "officeDocument", "word/document.xml", false), ("data", "customXml", "customXml/item1.xml", false)]).into_bytes(),
    );
    p.insert(
        "word/_rels/document.xml.rels".into(),
        rels(&[("one", "customXml", "../customXml/item1.xml", false), ("two", "customXml", "../customXml/other.xml", false)]).into_bytes(),
    );
    p.insert("customXml/other.xml".into(), b"<other>Independent store</other>".to_vec());
    p.insert("customXml/_rels/other.xml.rels".into(), rels(&[("shared", "customXmlProps", "itemProps1.xml", false)]).into_bytes());
    // A cycle must not cause repeated parts, and an external schema stays external.
    p.insert(
        "customXml/_rels/itemProps1.xml.rels".into(),
        rels(&[("cycle", "customXml", "item1.xml", false), ("schema", "schema", "https://example.invalid/schema", true)]).into_bytes(),
    );
    edited_cycles(&p, |out| {
        assert_eq!(out.get("customXml/item1.xml").map(Vec::as_slice), Some(DATA.as_bytes()));
        assert_eq!(out.get("customXml/other.xml"), p.get("customXml/other.xml"));
        assert!(text(out, "_rels/.rels").contains(r#"Target="customXml/item1.xml""#));
        assert!(text(out, "word/_rels/document.xml.rels").contains(r#"Target="../customXml/other.xml""#));
        assert!(text(out, "customXml/_rels/itemProps1.xml.rels").contains(r#"TargetMode="External""#));
        assert!(text(out, "customXml/_rels/other.xml.rels").contains(r#"Target="itemProps1.xml""#));
    });
}

#[test]
fn absent_and_external_stores_are_not_fetched_or_written() {
    let mut p = fixture();
    p.insert(
        "word/_rels/document.xml.rels".into(),
        rels(&[
            ("missing", "customXml", "../customXml/missing.xml", false),
            ("external", "customXml", "https://example.invalid/store.xml", true),
            ("valid", "customXml", "../customXml/item1.xml", false),
        ])
        .into_bytes(),
    );
    p.insert("customXml/_rels/item1.xml.rels".into(), rels(&[("missing", "customXmlProps", "missing.xml", false)]).into_bytes());
    edited_cycles(&p, |out| {
        assert_eq!(out.get("customXml/item1.xml").map(Vec::as_slice), Some(DATA.as_bytes()));
        let roots = text(out, "word/_rels/document.xml.rels");
        assert!(!roots.contains("missing.xml"));
        assert!(!roots.contains("example.invalid"));
        assert!(!out.contains_key("customXml/_rels/item1.xml.rels"));
    });
}

#[test]
fn custom_stores_do_not_shadow_reserved_bibliography_names() {
    let mut doc = wordcraft_docx::read(&zip(&fixture())).unwrap();
    doc.sources.push(wordcraft_doc::Source { tag: "OriginalSource".into(), title: "Original synthetic reference".into(), ..Default::default() });
    let out = unzip(&wordcraft_docx::write(&doc).unwrap());
    assert_eq!(out.get("customXml/item1_2.xml").map(Vec::as_slice), Some(DATA.as_bytes()));
    assert_eq!(out.get("customXml/itemProps1_2.xml").map(Vec::as_slice), Some(PROPS.as_bytes()));
    assert!(text(&out, "word/_rels/document.xml.rels").contains(r#"Target="../customXml/item1_2.xml""#));
    assert!(text(&out, "customXml/_rels/item1_2.xml.rels").contains(r#"Target="itemProps1_2.xml""#));
}

#[test]
fn opaque_non_utf8_data_and_strict_relationship_types_are_preserved() {
    let mut p = fixture();
    let data: Vec<u8> =
        "<?xml version=\"1.0\" encoding=\"UTF-16\"?><customer>Atlas Studios</customer>".encode_utf16().flat_map(u16::to_le_bytes).collect();
    let mut bytes = vec![0xff, 0xfe];
    bytes.extend(data);
    p.insert("customXml/item1.xml".into(), bytes.clone());
    let strict =
        rels(&[("strict", "customXml", "../customXml/item1.xml", false)]).replace(RT, "http://purl.oclc.org/ooxml/officeDocument/relationships/");
    p.insert("word/_rels/document.xml.rels".into(), strict.into_bytes());
    edited_cycles(&p, |out| {
        assert_eq!(out.get("customXml/item1.xml"), Some(&bytes));
        assert!(text(out, "word/_rels/document.xml.rels").contains("http://purl.oclc.org/ooxml/officeDocument/relationships/customXml"));
    });
}

#[test]
fn reserved_part_targets_are_renamed_instead_of_shadowing_or_looping() {
    let mut p = fixture();
    p.insert("word/_rels/document.xml.rels".into(), rels(&[("hostile", "customXml", "../docProps/foreign.xml", false)]).into_bytes());
    p.insert("docProps/foreign.xml".into(), DATA.as_bytes().to_vec());
    edited_cycles(&p, |out| {
        assert!(out.values().any(|b| b == DATA.as_bytes()));
        assert!(!out.contains_key("docProps/foreign.xml"));
        assert!(text(out, "word/_rels/document.xml.rels").contains(r#"Target="embeddings/part.xml""#));
        assert!(text(out, "docProps/core.xml").contains("coreProperties"));
    });
}

// InkWitness: custom XML parts set and found by namespace (`set_custom_xml_part`).

const NS_A: &str = "urn:example:evidence:a";
const NS_B: &str = "urn:example:evidence:b";
const ID_A: &str = "{AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA}";
const PROPS_CT: &str = "application/vnd.openxmlformats-officedocument.customXmlProperties+xml";

fn store(ns: &str, body: &str) -> Vec<u8> {
    format!("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<x:store xmlns:x=\"{ns}\">{body}</x:store>").into_bytes()
}

fn new_doc() -> wordcraft_doc::Document {
    let mut doc = wordcraft_doc::Document::new();
    doc.para_mut(StoryRef::Body, &Path::top(0)).unwrap().insert_text(0, "Example Student essay.", &Default::default()).unwrap();
    doc
}

fn found(doc: &wordcraft_doc::Document, ns: &str) -> Vec<u8> {
    wordcraft_docx::find_custom_xml_part(doc, ns).unwrap().expect("the part is found").bytes.to_vec()
}

/// Resolve a relationship target from the part that owns `rels_name`.
fn rel_source_dir(rels_name: &str) -> String {
    let source = rels_name.replace("_rels/", "").trim_end_matches(".rels").to_string();
    source.rsplit_once('/').map_or(String::new(), |(d, _)| d.to_string())
}

fn resolve(dir: &str, target: &str) -> String {
    let mut parts: Vec<&str> = if target.starts_with('/') { Vec::new() } else { dir.split('/').filter(|s| !s.is_empty()).collect() };
    for seg in target.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            s => parts.push(s),
        }
    }
    parts.join("/")
}

/// Every relationship target in the package.
fn targets(p: &BTreeMap<String, Vec<u8>>) -> Vec<String> {
    let mut out = Vec::new();
    for (name, bytes) in p.iter().filter(|(n, _)| n.ends_with(".rels")) {
        let dir = rel_source_dir(name);
        let s = String::from_utf8(bytes.clone()).unwrap();
        for rel in s.split("<Relationship ").skip(1) {
            if rel.contains("TargetMode=\"External\"") {
                continue;
            }
            let t = rel.split("Target=\"").nth(1).unwrap().split('"').next().unwrap();
            out.push(resolve(&dir, t));
        }
    }
    out
}

/// Every part is related (no orphans), every internal relationship has its part, and each
/// `customXml/` part has a content type.
fn assert_package_consistent(p: &BTreeMap<String, Vec<u8>>) {
    let targets = targets(p);
    for name in p.keys().filter(|n| !n.ends_with(".rels") && *n != "[Content_Types].xml") {
        assert!(targets.contains(name), "orphan part {name}");
    }
    for t in &targets {
        assert!(p.contains_key(t), "relationship to missing part {t}");
    }
    let types = text(p, "[Content_Types].xml");
    for name in p.keys().filter(|n| n.starts_with("customXml/itemProps")) {
        assert!(types.contains(&format!(r#"<Override PartName="/{name}" ContentType="{PROPS_CT}"/>"#)), "{name} content type");
    }
}

fn custom_parts(p: &BTreeMap<String, Vec<u8>>) -> Vec<&String> {
    p.keys().filter(|n| n.starts_with("customXml/") && !n.contains("_rels")).collect()
}

#[test]
fn a_part_set_by_namespace_is_written_with_properties_and_read_back_byte_for_byte() {
    let mut doc = new_doc();
    let data = store(NS_A, "<x:n>1</x:n>");
    let set = wordcraft_docx::set_custom_xml_part_with_id(&mut doc, NS_A, &data, ID_A).unwrap();
    assert_eq!((set.part.as_str(), set.item_id.as_deref()), ("customXml/item1.xml", Some(ID_A)));
    assert_eq!(found(&doc, NS_A), data, "found before saving");

    let bytes = wordcraft_docx::write(&doc).unwrap();
    let p = unzip(&bytes);
    assert_eq!(p.get("customXml/item1.xml"), Some(&data));
    let props = text(&p, "customXml/itemProps1.xml");
    assert!(props.contains(r#"xmlns:ds="http://schemas.openxmlformats.org/officeDocument/2006/customXml""#));
    assert!(props.contains(&format!(r#"ds:itemID="{ID_A}""#)));
    assert!(props.contains(&format!(r#"<ds:schemaRef ds:uri="{NS_A}"/>"#)));
    assert!(text(&p, "word/_rels/document.xml.rels").contains(&format!(r#"Type="{RT}customXml" Target="../customXml/item1.xml""#)));
    assert!(text(&p, "customXml/_rels/item1.xml.rels").contains(&format!(r#"Type="{RT}customXmlProps" Target="itemProps1.xml""#)));
    assert!(text(&p, "[Content_Types].xml").contains(r#"<Override PartName="/customXml/item1.xml" ContentType="application/xml"/>"#));
    assert_package_consistent(&p);

    let back = wordcraft_docx::read(&bytes).unwrap();
    let part = wordcraft_docx::find_custom_xml_part(&back, NS_A).unwrap().unwrap();
    assert_eq!(part.bytes.as_slice(), data.as_slice());
    assert_eq!(part.item_id.as_deref(), Some(ID_A));
    assert_eq!(wordcraft_docx::find_custom_xml_part(&back, NS_B), Ok(None));
}

#[test]
fn setting_a_namespace_again_replaces_its_part_in_place() {
    let mut doc = new_doc();
    wordcraft_docx::set_custom_xml_part_with_id(&mut doc, NS_A, &store(NS_A, "old"), ID_A).unwrap();
    let mut doc = wordcraft_docx::read(&wordcraft_docx::write(&doc).unwrap()).unwrap();
    let newer = store(NS_A, "new");
    // A different id for a new properties part: the existing one must be kept instead.
    let set = wordcraft_docx::set_custom_xml_part_with_id(&mut doc, NS_A, &newer, "{BBBBBBBB-BBBB-4BBB-8BBB-BBBBBBBBBBBB}").unwrap();
    assert_eq!((set.part.as_str(), set.item_id.as_deref()), ("customXml/item1.xml", Some(ID_A)));
    let p = unzip(&wordcraft_docx::write(&doc).unwrap());
    assert_eq!(custom_parts(&p), ["customXml/item1.xml", "customXml/itemProps1.xml"], "no second copy");
    assert_eq!(p.get("customXml/item1.xml"), Some(&newer));
    assert_eq!(text(&p, "word/_rels/document.xml.rels").matches("customXml/item").count(), 1);
    assert_package_consistent(&p);
}

#[test]
fn a_set_part_survives_edited_saves() {
    let mut doc = new_doc();
    let data = store(NS_A, "kept");
    wordcraft_docx::set_custom_xml_part_with_id(&mut doc, NS_A, &data, ID_A).unwrap();
    let input = unzip(&wordcraft_docx::write(&doc).unwrap());
    edited_cycles(&input, |p| {
        assert_eq!(custom_parts(p), ["customXml/item1.xml", "customXml/itemProps1.xml"]);
        assert_eq!(p.get("customXml/item1.xml"), Some(&data));
        assert!(text(p, "word/document.xml").contains("Unrelated edit. "));
        assert_package_consistent(p);
        assert_eq!(found(&wordcraft_docx::read(&zip(p)).unwrap(), NS_A), data);
    });
}

#[test]
fn parts_in_different_namespaces_coexist_with_a_stranger_store() {
    // Start from a file that already holds someone else's store at `item1.xml`.
    let mut doc = wordcraft_docx::read(&zip(&fixture())).unwrap();
    let (a, b) = (store(NS_A, "a"), store(NS_B, "b"));
    let pa = wordcraft_docx::set_custom_xml_part(&mut doc, NS_A, &a).unwrap();
    let pb = wordcraft_docx::set_custom_xml_part(&mut doc, NS_B, &b).unwrap();
    assert_eq!((pa.part.as_str(), pb.part.as_str()), ("customXml/item2.xml", "customXml/item3.xml"));
    assert_ne!(pa.item_id, pb.item_id, "each new part gets its own random id");
    let back = wordcraft_docx::read(&wordcraft_docx::write(&doc).unwrap()).unwrap();
    assert_eq!((found(&back, NS_A), found(&back, NS_B)), (a.clone(), b));
    assert_eq!(found(&back, "urn:original:customer"), DATA.as_bytes());
    let mut back = back;
    assert_eq!(wordcraft_docx::remove_custom_xml_part(&mut back, NS_A), 1);
    let p = unzip(&wordcraft_docx::write(&back).unwrap());
    assert!(!p.values().any(|v| *v == a), "removed");
    assert_eq!(custom_parts(&p).len(), 4, "the other two stores and their properties stay");
    assert_package_consistent(&p);
}

#[test]
fn the_bibliography_part_does_not_clash_with_a_set_part() {
    let mut doc = new_doc();
    doc.sources.push(wordcraft_doc::Source { tag: "Example2026".into(), title: "Synthetic reference".into(), ..Default::default() });
    let data = store(NS_A, "with sources");
    assert_eq!(wordcraft_docx::set_custom_xml_part(&mut doc, NS_A, &data).unwrap().part, "customXml/item2.xml");
    let bytes = wordcraft_docx::write(&doc).unwrap();
    let p = unzip(&bytes);
    assert!(text(&p, "customXml/item1.xml").contains("Sources"), "the bibliography keeps item1");
    assert_eq!(p.get("customXml/item2.xml"), Some(&data));
    assert_package_consistent(&p);
    let back = wordcraft_docx::read(&bytes).unwrap();
    assert_eq!(back.sources.len(), 1);
    assert_eq!(found(&back, NS_A), data);
    // Sources added after the part was set: the writer renames it, the namespace still finds it.
    let mut doc = new_doc();
    wordcraft_docx::set_custom_xml_part(&mut doc, NS_A, &data).unwrap();
    doc.sources.push(wordcraft_doc::Source { tag: "Late2026".into(), title: "Synthetic reference".into(), ..Default::default() });
    let bytes = wordcraft_docx::write(&doc).unwrap();
    assert_package_consistent(&unzip(&bytes));
    let back = wordcraft_docx::read(&bytes).unwrap();
    assert_eq!((back.sources.len(), found(&back, NS_A)), (1, data));
}

#[test]
fn parts_are_found_by_namespace_not_name_and_duplicates_are_ambiguous() {
    let mut p = fixture();
    p.insert(
        "word/_rels/document.xml.rels".into(),
        rels(&[
            ("one", "customXml", "../customXml/item1.xml", false),
            ("two", "customXml", "../custom/zz.xml", false),
            ("three", "customXml", "../custom/yy.xml", false),
        ])
        .into_bytes(),
    );
    p.insert("custom/zz.xml".into(), store(NS_A, "first"));
    p.insert("custom/yy.xml".into(), store(NS_A, "second"));
    p.insert("custom/_rels/yy.xml.rels".into(), rels(&[("p", "customXmlProps", "yyProps.xml", false)]).into_bytes());
    p.insert("custom/yyProps.xml".into(), PROPS.replace("urn:original:customer", NS_A).into_bytes());
    let mut doc = wordcraft_docx::read(&zip(&p)).unwrap();
    assert_eq!(wordcraft_docx::custom_xml_parts(&doc, NS_A).len(), 2);
    assert!(matches!(wordcraft_docx::find_custom_xml_part(&doc, NS_A), Err(wordcraft_docx::CustomXmlError::Ambiguous { count: 2, .. })));
    // Setting resolves it: the first by part name is kept, the other removed with its properties.
    let data = store(NS_A, "only");
    let set = wordcraft_docx::set_custom_xml_part(&mut doc, NS_A, &data).unwrap();
    assert_eq!(set.part, "custom/yy.xml");
    assert_eq!(set.item_id.as_deref(), Some("{22222222-2222-2222-2222-222222222222}"));
    let out = unzip(&wordcraft_docx::write(&doc).unwrap());
    assert_eq!(out.get("custom/yy.xml"), Some(&data));
    assert!(!out.contains_key("custom/zz.xml"));
    assert_package_consistent(&out);
    // `custom/zz.xml` had no properties part; set on it alone gets one.
    let mut p2 = p.clone();
    p2.remove("custom/yy.xml");
    let mut doc = wordcraft_docx::read(&zip(&p2)).unwrap();
    let set = wordcraft_docx::set_custom_xml_part_with_id(&mut doc, NS_A, &data, ID_A).unwrap();
    assert_eq!((set.part.as_str(), set.item_id.as_deref()), ("custom/zz.xml", Some(ID_A)));
    let out = unzip(&wordcraft_docx::write(&doc).unwrap());
    assert_package_consistent(&out);
    assert!(text(&out, "custom/_rels/zz.xml.rels").contains("customXmlProps"));
}

#[test]
fn hostile_parts_and_arguments_are_refused_without_panicking() {
    use wordcraft_docx::CustomXmlError as E;
    let mut doc = new_doc();
    let set = |doc: &mut wordcraft_doc::Document, ns: &str, xml: &[u8]| wordcraft_docx::set_custom_xml_part(doc, ns, xml);
    assert_eq!(set(&mut doc, NS_A, &store(NS_B, "x")), Err(E::WrongNamespace(NS_A.into())));
    assert_eq!(set(&mut doc, NS_A, b"not xml"), Err(E::WrongNamespace(NS_A.into())));
    assert_eq!(set(&mut doc, NS_A, b"<store>no namespace</store>"), Err(E::WrongNamespace(NS_A.into())));
    let dtd =
        format!(r#"<?xml version="1.0"?><!DOCTYPE x:store [<!ENTITY e SYSTEM "https://example.invalid/e">]><x:store xmlns:x="{NS_A}">&e;</x:store>"#);
    assert_eq!(set(&mut doc, NS_A, dtd.as_bytes()), Err(E::WrongNamespace(NS_A.into())), "a document type is never read");
    assert_eq!(set(&mut doc, "", b"<a/>"), Err(E::BadNamespace(String::new())));
    assert_eq!(set(&mut doc, "urn:a\tb", b"<a/>"), Err(E::BadNamespace("urn:a\tb".into())));
    assert!(matches!(wordcraft_docx::set_custom_xml_part_with_id(&mut doc, NS_A, &store(NS_A, ""), "AAAA"), Err(E::BadItemId(_))));
    let big = vec![b' '; wordcraft_docx::MAX_CUSTOM_XML_PART + 1];
    assert!(matches!(set(&mut doc, NS_A, &big), Err(E::TooLarge(_))));
    assert!(doc.passthrough.is_empty(), "nothing was added");

    // A file whose store in the namespace declares a DTD, or is only reachable externally.
    let mut p = fixture();
    p.insert(
        "word/_rels/document.xml.rels".into(),
        rels(&[("dtd", "customXml", "../customXml/item1.xml", false), ("ext", "customXml", "https://example.invalid/store.xml", true)]).into_bytes(),
    );
    p.insert("customXml/item1.xml".into(), dtd.into_bytes());
    let doc = wordcraft_docx::read(&zip(&p)).unwrap();
    assert_eq!(wordcraft_docx::find_custom_xml_part(&doc, NS_A), Ok(None));
}
