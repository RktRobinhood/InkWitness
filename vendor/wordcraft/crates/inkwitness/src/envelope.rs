//! The evidence envelope: the `<wa:history>` custom XML part a DOCX carries the evidence JSON in
//! (`docs/DOCX_FORMAT.md`, InkWitness repository).
//!
//! ```xml
//! <?xml version="1.0" encoding="utf-8"?>
//! <wa:history xmlns:wa="urn:workalone:evidence:v1" schemaVersion="1.0"
//!     payloadMediaType="application/vnd.workalone.history+json" encoding="gzip+base64"
//!     attemptId="…">BASE64_OF_GZIPPED_EVIDENCE_JSON</wa:history>
//! ```
//!
//! The part is found by its namespace ([`NAMESPACE`]), never by file name. Reading is bounded
//! ([`MAX_ENVELOPE_BYTES`] of XML, [`MAX_PAYLOAD_BYTES`] of JSON once decompressed), refuses a
//! document type declaration and any entity reference, follows no relationship, and never panics:
//! hostile input is an [`EnvelopeError`].

use std::io::Read;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use flate2::Compression;
use flate2::bufread::GzDecoder;
use flate2::write::GzEncoder;
use quick_xml::NsReader;
use quick_xml::events::Event;
use quick_xml::name::ResolveResult;
use serde::Deserialize;
use wordcraft_doc::Document;
use wordcraft_docx::{CustomXmlError, CustomXmlPart};

/// The envelope's XML namespace (a protocol identifier, not public branding; `DOCX_FORMAT.md`).
pub const NAMESPACE: &str = "urn:workalone:evidence:v1";
/// The envelope's root element (local name).
pub const ROOT: &str = "history";
/// The one `schemaVersion` this reader takes.
pub const SCHEMA_VERSION: &str = "1.0";
/// The payload's media type.
pub const PAYLOAD_MEDIA_TYPE: &str = "application/vnd.workalone.history+json";
/// The payload's encoding.
pub const ENCODING: &str = "gzip+base64";
/// Largest evidence JSON, uncompressed.
pub const MAX_PAYLOAD_BYTES: usize = 64 * 1024 * 1024;
/// Largest envelope part (the XML, base64 text included).
pub const MAX_ENVELOPE_BYTES: usize = 32 * 1024 * 1024;
/// Prolog events (declaration, comments, whitespace) read before the root element.
const MAX_PROLOG_EVENTS: usize = 64;

/// Why an envelope can't be written or read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvelopeError {
    /// The envelope part is larger than [`MAX_ENVELOPE_BYTES`].
    EnvelopeTooLarge(usize),
    /// The payload is (or decompresses to) more than [`MAX_PAYLOAD_BYTES`].
    PayloadTooLarge,
    /// The XML declares a document type (never read: no DTD, no entities).
    DocumentType,
    /// The root element is not `history` in [`NAMESPACE`].
    WrongNamespace,
    /// `schemaVersion`, `encoding` or `payloadMediaType` is missing or not one this reader takes.
    Unsupported(String),
    /// The XML is malformed, has an entity reference, child elements or a bad `attemptId`.
    Malformed(String),
    /// The payload text is not base64.
    BadBase64,
    /// The decoded payload is not one complete gzip stream.
    BadGzip,
    /// The decompressed payload is not a JSON object.
    BadJson(String),
    /// The payload's `attemptId` differs from the envelope's.
    AttemptMismatch,
    /// More than one part in the document is in [`NAMESPACE`].
    Ambiguous(usize),
    /// The DOCX helper refused the part.
    Part(String),
}

impl std::fmt::Display for EnvelopeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EnvelopeError::EnvelopeTooLarge(n) => write!(f, "the evidence part is {n} bytes, more than {MAX_ENVELOPE_BYTES}"),
            EnvelopeError::PayloadTooLarge => write!(f, "the evidence is larger than {MAX_PAYLOAD_BYTES} bytes"),
            EnvelopeError::DocumentType => write!(f, "the evidence part declares a document type"),
            EnvelopeError::WrongNamespace => write!(f, "the evidence part's root is not {NAMESPACE} {ROOT}"),
            EnvelopeError::Unsupported(what) => write!(f, "unsupported evidence envelope: {what}"),
            EnvelopeError::Malformed(what) => write!(f, "malformed evidence envelope: {what}"),
            EnvelopeError::BadBase64 => write!(f, "the evidence payload is not base64"),
            EnvelopeError::BadGzip => write!(f, "the evidence payload is not a gzip stream"),
            EnvelopeError::BadJson(e) => write!(f, "the evidence payload is not a JSON object: {e}"),
            EnvelopeError::AttemptMismatch => write!(f, "the evidence payload's attemptId differs from the envelope's"),
            EnvelopeError::Ambiguous(n) => write!(f, "{n} evidence parts are in the document"),
            EnvelopeError::Part(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for EnvelopeError {}

impl From<CustomXmlError> for EnvelopeError {
    fn from(e: CustomXmlError) -> Self {
        match e {
            CustomXmlError::Ambiguous { count, .. } => EnvelopeError::Ambiguous(count),
            CustomXmlError::TooLarge(n) => EnvelopeError::EnvelopeTooLarge(n),
            e => EnvelopeError::Part(e.to_string()),
        }
    }
}

/// A read envelope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Envelope {
    pub attempt_id: String,
    pub schema_version: String,
    /// The evidence JSON, exactly the bytes that were written.
    pub payload: Vec<u8>,
}

/// The one payload field the envelope checks.
#[derive(Deserialize)]
struct Head {
    #[serde(rename = "attemptId")]
    attempt_id: Option<serde_json::Value>,
}

/// Is `s` a UUID (`8-4-4-4-12` hex digits)?
pub fn is_uuid(s: &str) -> bool {
    let groups: Vec<&str> = s.split('-').collect();
    groups.len() == 5 && groups.iter().zip([8, 4, 4, 4, 12]).all(|(g, n)| g.len() == n && g.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// Check that `payload` is a JSON object whose `attemptId`, if it has one, is `attempt_id`.
fn check_payload(payload: &[u8], attempt_id: &str) -> Result<(), EnvelopeError> {
    let head: Head = serde_json::from_slice(payload).map_err(|e| EnvelopeError::BadJson(e.to_string()))?;
    match head.attempt_id {
        None => Ok(()),
        Some(serde_json::Value::String(a)) if a == attempt_id => Ok(()),
        Some(_) => Err(EnvelopeError::AttemptMismatch),
    }
}

/// The envelope part for evidence JSON `payload` of attempt `attempt_id`. The gzip stream is
/// deterministic (no file name, time 0), so the same payload gives the same bytes.
pub fn encode(attempt_id: &str, payload: &[u8]) -> Result<Vec<u8>, EnvelopeError> {
    if !is_uuid(attempt_id) {
        return Err(EnvelopeError::Malformed(format!("attemptId {attempt_id:?} is not a UUID")));
    }
    if payload.len() > MAX_PAYLOAD_BYTES {
        return Err(EnvelopeError::PayloadTooLarge);
    }
    check_payload(payload, attempt_id)?;
    let mut gz = GzEncoder::new(Vec::new(), Compression::best());
    std::io::Write::write_all(&mut gz, payload).map_err(|_| EnvelopeError::BadGzip)?;
    let compressed = gz.finish().map_err(|_| EnvelopeError::BadGzip)?;
    let text = STANDARD.encode(compressed);
    let xml = format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<wa:history xmlns:wa=\"{NAMESPACE}\" schemaVersion=\"{SCHEMA_VERSION}\" payloadMediaType=\"{PAYLOAD_MEDIA_TYPE}\" encoding=\"{ENCODING}\" attemptId=\"{attempt_id}\">{text}</wa:history>"
    );
    if xml.len() > MAX_ENVELOPE_BYTES {
        return Err(EnvelopeError::EnvelopeTooLarge(xml.len()));
    }
    Ok(xml.into_bytes())
}

/// Read an envelope part.
pub fn decode(xml: &[u8]) -> Result<Envelope, EnvelopeError> {
    if xml.len() > MAX_ENVELOPE_BYTES {
        return Err(EnvelopeError::EnvelopeTooLarge(xml.len()));
    }
    // An editor may re-save the part as UTF-16 (with a byte order mark); a UTF-8 mark is dropped.
    let transcoded = utf16_to_utf8(xml)?;
    let xml = match &transcoded {
        Some(s) => s.as_bytes(),
        None => xml.strip_prefix(&[0xEF, 0xBB, 0xBF][..]).unwrap_or(xml),
    };
    let mut r = NsReader::from_reader(xml);
    r.config_mut().check_end_names = true;
    let malformed = |e: quick_xml::Error| EnvelopeError::Malformed(e.to_string());
    let mut buf = Vec::new();

    // The prolog, then the root element and its attributes.
    let mut prolog = 0usize;
    let (attempt_id, schema_version) = loop {
        prolog += 1;
        if prolog > MAX_PROLOG_EVENTS {
            return Err(EnvelopeError::Malformed("no root element".into()));
        }
        buf.clear();
        let (res, ev) = r.read_resolved_event_into(&mut buf).map_err(malformed)?;
        let e = match ev {
            Event::Decl(_) | Event::Comment(_) => continue,
            Event::Text(t) if t.iter().all(u8::is_ascii_whitespace) => continue,
            Event::DocType(_) => return Err(EnvelopeError::DocumentType),
            Event::Start(e) => e,
            Event::Empty(_) => return Err(EnvelopeError::Malformed("the envelope has no payload".into())),
            _ => return Err(EnvelopeError::Malformed("unexpected content before the root element".into())),
        };
        let in_ns = matches!(&res, ResolveResult::Bound(ns) if ns.as_ref() == NAMESPACE.as_bytes());
        if !in_ns || e.local_name().as_ref() != ROOT.as_bytes() {
            return Err(EnvelopeError::WrongNamespace);
        }
        let (mut attempt, mut version, mut encoding, mut media) = (None, None, None, None);
        for a in e.attributes() {
            let a = a.map_err(|e| EnvelopeError::Malformed(e.to_string()))?;
            // The envelope's own attributes are unqualified.
            let slot = match a.key.as_ref() {
                b"attemptId" => &mut attempt,
                b"schemaVersion" => &mut version,
                b"encoding" => &mut encoding,
                b"payloadMediaType" => &mut media,
                _ => continue,
            };
            // Only the five predefined entities and character references: never a declared one.
            let v = a.normalized_value(quick_xml::XmlVersion::Explicit1_0).map_err(|e| EnvelopeError::Malformed(e.to_string()))?;
            *slot = Some(v.into_owned());
        }
        match (&version, &encoding, &media) {
            (Some(v), _, _) if v != SCHEMA_VERSION => return Err(EnvelopeError::Unsupported(format!("schemaVersion {v:?}"))),
            (None, _, _) => return Err(EnvelopeError::Unsupported("no schemaVersion".into())),
            (_, e, _) if e.as_deref() != Some(ENCODING) => return Err(EnvelopeError::Unsupported(format!("encoding {e:?}"))),
            (_, _, m) if m.as_deref() != Some(PAYLOAD_MEDIA_TYPE) => return Err(EnvelopeError::Unsupported(format!("payloadMediaType {m:?}"))),
            _ => {}
        }
        let attempt = attempt.filter(|a| is_uuid(a)).ok_or_else(|| EnvelopeError::Malformed("attemptId is missing or not a UUID".into()))?;
        break (attempt, version.unwrap_or_default());
    };

    // The payload text: base64 and whitespace only, then the end of the root and of the data.
    let mut text: Vec<u8> = Vec::new();
    loop {
        buf.clear();
        match r.read_event_into(&mut buf).map_err(malformed)? {
            Event::Text(t) => {
                if !t.iter().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'=') || b.is_ascii_whitespace()) {
                    return Err(EnvelopeError::BadBase64);
                }
                text.extend(t.iter().filter(|b| !b.is_ascii_whitespace()));
            }
            Event::Comment(_) => {}
            Event::End(_) => break,
            Event::GeneralRef(_) => return Err(EnvelopeError::Malformed("an entity reference in the payload".into())),
            Event::Eof => return Err(EnvelopeError::Malformed("the root element is not closed".into())),
            _ => return Err(EnvelopeError::Malformed("unexpected markup in the payload".into())),
        }
    }
    loop {
        buf.clear();
        match r.read_event_into(&mut buf).map_err(malformed)? {
            Event::Eof => break,
            Event::Comment(_) => {}
            Event::Text(t) if t.iter().all(u8::is_ascii_whitespace) => {}
            _ => return Err(EnvelopeError::Malformed("content after the root element".into())),
        }
    }

    let compressed = STANDARD.decode(&text).map_err(|_| EnvelopeError::BadBase64)?;
    let payload = gunzip(&compressed)?;
    check_payload(&payload, &attempt_id)?;
    Ok(Envelope { attempt_id, schema_version, payload })
}

/// `bytes` as UTF-8 when they start with a UTF-16 byte order mark; `None` when they don't.
fn utf16_to_utf8(bytes: &[u8]) -> Result<Option<String>, EnvelopeError> {
    let le = match bytes.get(..2) {
        Some([0xFF, 0xFE]) => true,
        Some([0xFE, 0xFF]) => false,
        _ => return Ok(None),
    };
    let rest = bytes.get(2..).unwrap_or_default();
    if rest.len() % 2 != 0 {
        return Err(EnvelopeError::Malformed("odd-length UTF-16".into()));
    }
    let units = rest.as_chunks::<2>().0.iter().map(|c| if le { u16::from_le_bytes(*c) } else { u16::from_be_bytes(*c) });
    char::decode_utf16(units).collect::<Result<String, _>>().map(Some).map_err(|_| EnvelopeError::Malformed("invalid UTF-16".into()))
}

/// Decompress one complete gzip stream of at most [`MAX_PAYLOAD_BYTES`].
fn gunzip(compressed: &[u8]) -> Result<Vec<u8>, EnvelopeError> {
    gunzip_bounded(compressed, MAX_PAYLOAD_BYTES)
}

/// Decompress one complete gzip stream of at most `max` bytes.
pub(crate) fn gunzip_bounded(compressed: &[u8], max: usize) -> Result<Vec<u8>, EnvelopeError> {
    let mut gz = GzDecoder::new(compressed);
    let mut out = Vec::new();
    let limit = u64::try_from(max).unwrap_or(u64::MAX).saturating_add(1);
    (&mut gz).take(limit).read_to_end(&mut out).map_err(|_| EnvelopeError::BadGzip)?;
    if out.len() > max {
        return Err(EnvelopeError::PayloadTooLarge);
    }
    // One stream, nothing after it (a second member or trailing bytes would go unread).
    if !gz.into_inner().is_empty() {
        return Err(EnvelopeError::BadGzip);
    }
    Ok(out)
}

/// Put the evidence JSON `payload` of attempt `attempt_id` into `doc` as its one envelope part
/// (replacing an earlier one in place).
pub fn embed(doc: &mut Document, attempt_id: &str, payload: &[u8]) -> Result<CustomXmlPart, EnvelopeError> {
    let xml = encode(attempt_id, payload)?;
    Ok(wordcraft_docx::set_custom_xml_part(doc, NAMESPACE, &xml)?)
}

/// [`embed`] with a fixed datastore item id (`{GUID}`) for a new part: reproducible output.
pub fn embed_with_item_id(doc: &mut Document, attempt_id: &str, payload: &[u8], item_id: &str) -> Result<CustomXmlPart, EnvelopeError> {
    let xml = encode(attempt_id, payload)?;
    Ok(wordcraft_docx::set_custom_xml_part_with_id(doc, NAMESPACE, &xml, item_id)?)
}

/// The envelope `doc` carries: `None` when it has no part in [`NAMESPACE`] (history missing),
/// an error when the part can't be read (history corrupt) or there are several.
pub fn extract(doc: &Document) -> Result<Option<Envelope>, EnvelopeError> {
    match wordcraft_docx::find_custom_xml_part(doc, NAMESPACE)? {
        None => Ok(None),
        Some(part) => decode(&part.bytes).map(Some),
    }
}
