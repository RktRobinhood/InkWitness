//! Custom XML data parts a caller owns, found by the **namespace** of their root element, never
//! by file name (ECMA-376 Part 1 §15.2.4 Custom XML Data Storage, §15.2.5 its Properties part).
//!
//! The parts ride in the same bookkeeping the reader keeps for custom XML stores it doesn't model
//! (`Document::passthrough`: the part bytes, [`CUSTOM_XML_ROOTS`] and the [`EMBEDDED_PARTS`]
//! manifest), so a part added here is written like one read from a file, and one read from a file
//! is found here. Setting a namespace's part again replaces its bytes under the same part name
//! and keeps its properties part (and so its datastore item id): never a second copy.
//!
//! Nothing here follows an external relationship or loads a DTD: a part whose prolog declares a
//! document type is not matched.

use std::sync::Arc;

use quick_xml::events::Event;
use quick_xml::name::ResolveResult;
use quick_xml::{NsReader, XmlVersion};
use sha2::{Digest, Sha256};
use wordcraft_doc::Document;

use crate::package::{CUSTOM_XML_ROOTS, EMBEDDED_PARTS, EmbeddedManifest, EmbeddedPart, MAX_EMBEDDED_RELS, Rel, manifest_field_ok, rel_is, rt};
use crate::xml::{W, utf16_to_utf8};

/// Largest custom XML part [`set_custom_xml_part`] accepts.
pub const MAX_CUSTOM_XML_PART: usize = 64 * 1024 * 1024;
/// Content type of a custom XML data part.
pub const CUSTOM_XML_CONTENT_TYPE: &str = "application/xml";
/// Content type of a custom XML properties part (`itemPropsN.xml`).
pub const CUSTOM_XML_PROPS_CONTENT_TYPE: &str = "application/vnd.openxmlformats-officedocument.customXmlProperties+xml";
/// The namespace of a custom XML properties part.
const DS_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/customXml";
/// Prolog events (declaration, comments, processing instructions) read before giving up on
/// finding a root element.
const MAX_PROLOG_EVENTS: usize = 64;

/// Why a custom XML part can't be set or found.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CustomXmlError {
    /// The namespace is empty or can't be stored.
    #[error("the namespace {0:?} can't name a custom XML part")]
    BadNamespace(String),
    /// The item id isn't a `{GUID}`.
    #[error("{0:?} is not a datastore item id ({{GUID}})")]
    BadItemId(String),
    /// The bytes are larger than [`MAX_CUSTOM_XML_PART`].
    #[error("the custom XML part is {0} bytes, more than {MAX_CUSTOM_XML_PART}")]
    TooLarge(usize),
    /// The bytes have no root element in the namespace (not XML, a document type declaration, or
    /// another namespace).
    #[error("the custom XML part's root element is not in namespace {0:?}")]
    WrongNamespace(String),
    /// More than one part has a root element in the namespace.
    #[error("{count} custom XML parts are in namespace {namespace:?}")]
    Ambiguous { namespace: String, count: usize },
}

/// A custom XML data part kept in a document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomXmlPart {
    /// Its part name (`customXml/item2.xml`): where it was read from, or will be written to
    /// unless that name is taken when saving.
    pub part: String,
    /// The `ds:itemID` of its properties part, when it has one.
    pub item_id: Option<String>,
    /// Its bytes.
    pub bytes: Arc<Vec<u8>>,
}

/// Every custom XML data part of `doc` whose root element is in `namespace`, in part-name order.
pub fn custom_xml_parts(doc: &Document, namespace: &str) -> Vec<CustomXmlPart> {
    let manifest = manifest(doc);
    let mut out: Vec<CustomXmlPart> = Vec::new();
    for target in roots(doc).into_iter().map(|(_, _, target)| target) {
        if out.iter().any(|p| p.part == target) || !manifest.parts.contains_key(&target) {
            continue;
        }
        let Some(bytes) = doc.passthrough.get(&target) else { continue };
        if root_element(bytes).is_some_and(|r| r.namespace == namespace) {
            let item_id = props_part(&manifest, &target).and_then(|p| doc.passthrough.get(&p)).and_then(|b| item_id_of(b));
            out.push(CustomXmlPart { part: target, item_id, bytes: bytes.clone() });
        }
    }
    out.sort_by(|a, b| a.part.cmp(&b.part));
    out
}

/// The one custom XML data part of `doc` whose root element is in `namespace`, if any.
pub fn find_custom_xml_part(doc: &Document, namespace: &str) -> Result<Option<CustomXmlPart>, CustomXmlError> {
    let mut found = custom_xml_parts(doc, namespace);
    match found.len() {
        0 => Ok(None),
        1 => Ok(found.pop()),
        count => Err(CustomXmlError::Ambiguous { namespace: namespace.to_string(), count }),
    }
}

/// Make `xml` (whose root element must be in `namespace`) the document's one custom XML part in
/// that namespace. An existing part keeps its name and properties part; a new one gets free
/// `customXml/itemN.xml` and `customXml/itemPropsN.xml` names, related from the main document,
/// and a fresh random datastore item id. Any further parts in the namespace are removed.
pub fn set_custom_xml_part(doc: &mut Document, namespace: &str, xml: &[u8]) -> Result<CustomXmlPart, CustomXmlError> {
    let id = random_item_id(namespace, xml);
    set_custom_xml_part_with_id(doc, namespace, xml, &id)
}

/// [`set_custom_xml_part`] with the datastore item id (`{GUID}`) a new properties part gets: for
/// output that must be reproducible. An existing properties part keeps its own id.
pub fn set_custom_xml_part_with_id(doc: &mut Document, namespace: &str, xml: &[u8], item_id: &str) -> Result<CustomXmlPart, CustomXmlError> {
    if !manifest_field_ok(namespace) || namespace.trim() != namespace {
        return Err(CustomXmlError::BadNamespace(namespace.to_string()));
    }
    if !is_item_id(item_id) {
        return Err(CustomXmlError::BadItemId(item_id.to_string()));
    }
    if xml.len() > MAX_CUSTOM_XML_PART {
        return Err(CustomXmlError::TooLarge(xml.len()));
    }
    if root_element(xml).is_none_or(|r| r.namespace != namespace) {
        return Err(CustomXmlError::WrongNamespace(namespace.to_string()));
    }
    let bytes = Arc::new(xml.to_vec());
    let found = custom_xml_parts(doc, namespace);
    for extra in found.iter().skip(1) {
        remove_tree(doc, &extra.part);
    }
    let mut manifest = manifest(doc);
    if let Some(first) = found.into_iter().next() {
        doc.passthrough.insert(first.part.clone(), bytes.clone());
        let kept = match first.item_id {
            Some(id) => id,
            None => {
                // No readable properties part: relate a new one instead of the broken one.
                drop_props(doc, &mut manifest, &first.part);
                let props = free_props_name(doc, &manifest);
                add_props(doc, &mut manifest, &first.part, &props, namespace, item_id);
                item_id.to_string()
            }
        };
        save_manifest(doc, &manifest);
        return Ok(CustomXmlPart { part: first.part, item_id: Some(kept), bytes });
    }
    let (part, props) = free_names(doc, &manifest);
    doc.passthrough.insert(part.clone(), bytes.clone());
    manifest.parts.insert(part.clone(), EmbeddedPart { content_type: CUSTOM_XML_CONTENT_TYPE.into(), rels: Vec::new() });
    add_props(doc, &mut manifest, &part, &props, namespace, item_id);
    save_manifest(doc, &manifest);
    let mut lines = doc.passthrough.get(CUSTOM_XML_ROOTS).map(|b| String::from_utf8_lossy(b).into_owned()).unwrap_or_default();
    if !lines.is_empty() && !lines.ends_with('\n') {
        lines.push('\n');
    }
    lines.push_str(&format!("D\t{}\t{part}\n", rt::CUSTOM_XML));
    doc.passthrough.insert(CUSTOM_XML_ROOTS.into(), Arc::new(lines.into_bytes()));
    Ok(CustomXmlPart { part, item_id: Some(item_id.to_string()), bytes })
}

/// Remove every custom XML data part of `doc` in `namespace`, with its properties part (unless
/// another part relates to that too). Returns how many were removed.
pub fn remove_custom_xml_part(doc: &mut Document, namespace: &str) -> usize {
    let found = custom_xml_parts(doc, namespace);
    for p in &found {
        remove_tree(doc, &p.part);
    }
    found.len()
}

/// The root element of an XML part: its namespace and its namespaced attributes.
struct Root {
    namespace: String,
    /// (attribute namespace, local name, value).
    attrs: Vec<(String, String, String)>,
}

/// Read only as far as the root element of `bytes`. `None` for malformed XML, an unbound root
/// prefix, or a document type declaration (never resolved).
fn root_element(bytes: &[u8]) -> Option<Root> {
    let transcoded = utf16_to_utf8(bytes);
    let bytes = match &transcoded {
        Some(s) => s.as_bytes(),
        None => bytes.strip_prefix(&[0xEF, 0xBB, 0xBF][..]).unwrap_or(bytes),
    };
    let mut r = NsReader::from_reader(bytes);
    let mut buf = Vec::new();
    for _ in 0..MAX_PROLOG_EVENTS {
        buf.clear();
        let (res, ev) = r.read_resolved_event_into(&mut buf).ok()?;
        let e = match ev {
            Event::Start(e) | Event::Empty(e) => e,
            Event::Decl(_) | Event::Comment(_) | Event::PI(_) => continue,
            Event::Text(t) if t.iter().all(u8::is_ascii_whitespace) => continue,
            _ => return None,
        };
        let ResolveResult::Bound(ns) = res else { return None };
        let namespace = String::from_utf8(ns.as_ref().to_vec()).ok()?;
        let mut attrs = Vec::new();
        for a in e.attributes().take(MAX_EMBEDDED_RELS) {
            let a = a.ok()?;
            let (ares, local) = r.resolver().resolve_attribute(a.key);
            let ans = match ares {
                ResolveResult::Bound(n) => String::from_utf8(n.as_ref().to_vec()).ok()?,
                _ => String::new(),
            };
            let local = String::from_utf8(local.as_ref().to_vec()).ok()?;
            let value = a.normalized_value(XmlVersion::Explicit1_0).ok()?.into_owned();
            attrs.push((ans, local, value));
        }
        return Some(Root { namespace, attrs });
    }
    None
}

/// The `ds:itemID` of a properties part.
fn item_id_of(bytes: &[u8]) -> Option<String> {
    let root = root_element(bytes).filter(|r| r.namespace == DS_NS)?;
    root.attrs.into_iter().find(|(ns, local, _)| ns == DS_NS && local == "itemID").map(|(_, _, v)| v)
}

/// Is `s` a `{GUID}`?
fn is_item_id(s: &str) -> bool {
    let Some(inner) = s.strip_prefix('{').and_then(|s| s.strip_suffix('}')) else { return false };
    let groups: Vec<&str> = inner.split('-').collect();
    groups.len() == 5 && groups.iter().zip([8, 4, 4, 4, 12]).all(|(g, n)| g.len() == n && g.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// A new version 4 GUID, `{XXXXXXXX-XXXX-4XXX-YXXX-XXXXXXXXXXXX}`. Without secure randomness it's
/// derived from the part (still a valid id; unique per namespace and content).
fn random_item_id(namespace: &str, xml: &[u8]) -> String {
    let mut b = [0u8; 16];
    if getrandom::fill(&mut b).is_err() {
        let mut h = Sha256::new();
        h.update(namespace.as_bytes());
        h.update([0]);
        h.update(xml);
        for (x, y) in b.iter_mut().zip(h.finalize().iter()) {
            *x = *y;
        }
    }
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h: String = b.iter().map(|x| format!("{x:02X}")).collect();
    format!("{{{}-{}-{}-{}-{}}}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32])
}

/// The properties part naming `namespace` as the item's schema.
fn props_xml(namespace: &str, item_id: &str) -> Vec<u8> {
    let mut w = W::new();
    w.open("ds:datastoreItem", &[("xmlns:ds", DS_NS), ("ds:itemID", item_id)]);
    w.open("ds:schemaRefs", &[]);
    w.empty("ds:schemaRef", &[("ds:uri", namespace)]);
    w.close("ds:schemaRefs");
    w.close("ds:datastoreItem");
    w.into_bytes()
}

fn manifest(doc: &Document) -> EmbeddedManifest {
    doc.passthrough.get(EMBEDDED_PARTS).map(|m| EmbeddedManifest::parse(m)).unwrap_or_default()
}

fn save_manifest(doc: &mut Document, manifest: &EmbeddedManifest) {
    if manifest.parts.is_empty() {
        doc.passthrough.remove(EMBEDDED_PARTS);
    } else {
        doc.passthrough.insert(EMBEDDED_PARTS.into(), Arc::new(manifest.to_bytes()));
    }
}

/// The custom XML roots kept for `doc`: (owner, relationship type, part).
fn roots(doc: &Document) -> Vec<(String, String, String)> {
    let Some(b) = doc.passthrough.get(CUSTOM_XML_ROOTS) else { return Vec::new() };
    String::from_utf8_lossy(b)
        .lines()
        .take(MAX_EMBEDDED_RELS * 2)
        .filter_map(|line| match line.split('\t').collect::<Vec<_>>().as_slice() {
            [owner, kind, target] if rel_is(kind, rt::CUSTOM_XML) => Some((owner.to_string(), kind.to_string(), target.to_string())),
            _ => None,
        })
        .collect()
}

/// The properties part `part` relates to.
fn props_part(manifest: &EmbeddedManifest, part: &str) -> Option<String> {
    manifest.parts.get(part)?.rels.iter().find(|r| !r.external && rel_is(&r.kind, rt::CUSTOM_XML_PROPS)).map(|r| r.target.clone())
}

/// Is part `name` (any case) in use?
fn taken(doc: &Document, manifest: &EmbeddedManifest, name: &str) -> bool {
    doc.passthrough.keys().chain(manifest.parts.keys()).any(|k| k.eq_ignore_ascii_case(name))
}

/// The first `n` (past the bibliography's `item1`, when the document has sources) whose item
/// and properties names are both free.
fn free_names(doc: &Document, manifest: &EmbeddedManifest) -> (String, String) {
    let mut n: u32 = if doc.sources.is_empty() { 1 } else { 2 };
    loop {
        let (item, props) = (format!("customXml/item{n}.xml"), format!("customXml/itemProps{n}.xml"));
        if !taken(doc, manifest, &item) && !taken(doc, manifest, &props) {
            return (item, props);
        }
        n = n.saturating_add(1);
    }
}

fn free_props_name(doc: &Document, manifest: &EmbeddedManifest) -> String {
    let mut n: u32 = if doc.sources.is_empty() { 1 } else { 2 };
    loop {
        let props = format!("customXml/itemProps{n}.xml");
        if !taken(doc, manifest, &props) {
            return props;
        }
        n = n.saturating_add(1);
    }
}

/// Write a properties part `props` for `part` and relate it.
fn add_props(doc: &mut Document, manifest: &mut EmbeddedManifest, part: &str, props: &str, namespace: &str, item_id: &str) {
    doc.passthrough.insert(props.to_string(), Arc::new(props_xml(namespace, item_id)));
    manifest.parts.insert(props.to_string(), EmbeddedPart { content_type: CUSTOM_XML_PROPS_CONTENT_TYPE.into(), rels: Vec::new() });
    if let Some(item) = manifest.parts.get_mut(part) {
        let mut n = 1u32;
        while item.rels.iter().any(|r| r.id == format!("rId{n}")) {
            n = n.saturating_add(1);
        }
        item.rels.push(Rel { id: format!("rId{n}"), kind: rt::CUSTOM_XML_PROPS.into(), target: props.to_string(), external: false });
    }
}

/// Unrelate `part`'s properties parts, removing each that nothing else relates to.
fn drop_props(doc: &mut Document, manifest: &mut EmbeddedManifest, part: &str) {
    let Some(item) = manifest.parts.get_mut(part) else { return };
    let (props, rest): (Vec<Rel>, Vec<Rel>) = item.rels.drain(..).partition(|r| !r.external && rel_is(&r.kind, rt::CUSTOM_XML_PROPS));
    item.rels = rest;
    for p in props {
        remove_if_unused(doc, manifest, &p.target);
    }
}

/// Remove `part` from the manifest and the document's bytes unless a kept part or root still
/// relates to it.
fn remove_if_unused(doc: &mut Document, manifest: &mut EmbeddedManifest, part: &str) {
    let related = manifest.parts.values().any(|p| p.rels.iter().any(|r| !r.external && r.target == part));
    if related || roots(doc).iter().any(|(_, _, t)| t == part) {
        return;
    }
    manifest.parts.remove(part);
    doc.passthrough.remove(part);
}

/// Remove custom XML root `part`, its relationships from both owners, and its properties part.
fn remove_tree(doc: &mut Document, part: &str) {
    if let Some(b) = doc.passthrough.get(CUSTOM_XML_ROOTS) {
        let kept: String = String::from_utf8_lossy(b).lines().filter(|l| l.split('\t').nth(2) != Some(part)).map(|l| format!("{l}\n")).collect();
        if kept.is_empty() {
            doc.passthrough.remove(CUSTOM_XML_ROOTS);
        } else {
            doc.passthrough.insert(CUSTOM_XML_ROOTS.into(), Arc::new(kept.into_bytes()));
        }
    }
    let mut manifest = manifest(doc);
    let rels = manifest.parts.get(part).map(|p| p.rels.clone()).unwrap_or_default();
    // Unrelate it first, so its own properties part is no longer counted as in use.
    manifest.parts.remove(part);
    doc.passthrough.remove(part);
    for r in rels.iter().filter(|r| !r.external && rel_is(&r.kind, rt::CUSTOM_XML_PROPS)) {
        remove_if_unused(doc, &mut manifest, &r.target);
    }
    save_manifest(doc, &manifest);
}
