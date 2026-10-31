# Portable DOCX evidence contract (proposed v1)

**Goal:** A document from InkWitness is a conventional Word-compatible `.docx`. Word can edit and display the normal content, tracked revisions and comments. An optional offline teacher reviewer reads additional InkWitness evidence embedded in OPC **Custom XML Parts**. No macros, no alternate proprietary file extension, no external document dependency.

## What is stored where

| Data | Standard Word location | InkWitness history location |
|---|---|---|
| Essay text/styles/tables/references | `word/document.xml`, styles, settings, relationships | Canonical captured document model at checkpoints |
| Native Word insertions/deletions | OOXML `w:ins`, `w:del`, etc., where WordCraft supports fidelity | Higher-granularity semantic operations and event timing |
| Standard review comments | `word/comments.xml` + document anchors | Diagnostic cards, rule/version, evidence and exact event references |
| Student identity and assignment | Normal header and `docProps/core.xml` / `docProps/custom.xml` (as appropriate) | `student`, `assignmentId`, `attemptId` |
| Faint watermark | Word/OOXML watermark with accessibility checked | Watermark config and identity consistency check |
| Writing replay, paste metadata, checkpoints, policy acknowledgement | No native Word equivalent | **`customXml/itemN.xml`** with a InkWitness namespace + associated OPC relationships |

The `customXml` directories and relationship filenames in examples are illustrative. The importer must find the **namespace and documented relationships**, not depend on `item1.xml` being a particular file. Distinguish Custom XML **Parts** from custom XML **markup in document text** (the latter is no longer supported by Word).

## Proposed embedded envelope (not Word's native tracking syntax)

```xml
<?xml version="1.0" encoding="utf-8"?>
<wa:history xmlns:wa="urn:workalone:evidence:v1"
    schemaVersion="1.0"
    payloadMediaType="application/vnd.workalone.history+json"
    encoding="gzip+base64"
    attemptId="00000000-0000-4000-8000-000000000001">
  BASE64_OF_GZIPPED_EVIDENCE_JSON
</wa:history>
```

Keep normal OPC relationships/content types for Custom XML item and its item-properties part. On export, **reopen the actual bytes** to test that the content is discoverable, the event chain reconstructs and the final recorded model matches. Uncompressed content size is bounded. If WordCraft already offers an opaque-part preservation mechanism, integrate without double-writing or orphaning relationships.

## Canonical document model and event integrity

- The replay event log includes durable `eventId`, chronological `seq`, `gestureId` for coordinated actions, `attemptId`, `sessionSpanId`, `fieldId`, monotonic time, wall-clock time, event type, committed reversible operations, and **anchors in document model units**. No display-pixel coordinates are serialized as authoritative anchors.
- Use **stable block/node identities** plus offset and context-digest to resolve a range at each intermediate model state. A paragraph split/merge can rebind anchors via the operations, not naive string index searches.
- Event chains link to previous event digest. Specify and freeze versioned canonical JSON serialization (RFC 8785 JCS or an equivalently cross-language deterministic format), domain-separated SHA-256 hashing and non-ambiguous `genesis` derivation after G0. Include test vectors that produce identical bytes/hashes in JS and Rust. Do not call an unimplemented placeholder an authenticated signature.
- Checkpoint every bounded number of committed operations or time interval and retain complete earlier event log; decompression and replay must produce the **same recorded final canonical state**. Store the canonicalization algorithm version. Distinguish true document changes from import normalization.
- The reviewer also compares the current open DOCX document against the recorded final state, reporting any unobserved changes since export as a **different category**, not an inferred paste.
- Editing/saving a DOCX in Word may change ZIP member ordering, relationship IDs and other OOXML details. Do not compare byte-for-byte self-hashes to decide whether it was edited; compare versioned canonical document meaning and explicitly state potential rendering/format changes.

## The self-hash limitation (non-negotiable)

A SHA-256 hash embedded *inside the same student-controlled document* is **not independent proof of integrity**. It is circular to hash the entire file including its own stored hash. It is feasible to hash a canonical representation of the recorded final document **excluding the evidence envelope**, then store this within the envelope. This catches many accidental or unsophisticated edits but **a determined person can rewrite both text and hashes**.

### Status taxonomy

| Status | Meaning | Important limit |
|---|---|---|
| `recorded_consistent` | Event chain reconstructs + recorded terminal state matches canonical Word content | Not independently authenticated |
| `external_changes` | Current DOCX no longer matches recorded terminal state | Later change occurred, but timing/source unknown |
| `history_missing` | No compatible InkWitness Custom XML found | Not proof of deliberate removal |
| `history_corrupt` | Invalid/missing/contradictory events, OPC or hash chain | Do not replay invented content |
| `receipt_not_available` | No independently captured original receipt to compare | Cannot certify post-export unchanged file |
| `receipt_match` | An original hash or digest held outside this document matches the received file/manifest | Validates comparison against that earlier record, not student authorship |
| `receipt_mismatch` | A separately retained digest does not match | Investigate, including legitimate Word resaves or file conversion |

For stronger transfer integrity without a new server, the school can independently retain the **original submitted DOCX** or ask the student to enter the export SHA-256 receipt into a timestamped LMS field at submission. This is **optional** and a second record; it does not change the default one-DOCX workflow. If the exact bytes are later modified by Word, an original byte hash will differ even for harmless changes; retain the original version for comparison.

A client-embedded key, static JavaScript secret or locally generated self-signature must never be described as a school-authenticated digital signature.

## External edits / reopening

- If a later Word save preserves Custom XML, retain prior writing-day history. It is **read-only evidence**, not a continuously updated source of new edits.
- Word's own revisions after export may be displayed separately, if present, but are not a replacement for observed InkWitness edit events.
- If a file is reimported to `/write/`, begin a **new recorded session span**. Imported prior text that lacks observable history is an `unobserved_import` snapshot. Never backfill fake typing events.
- A file reopened in `/write/` whose history is whole and the same student's **continues** it: old events and their chain stay as they were, and the new session is appended after them (ADR 0004). If its text no longer matches the last recorded state, the difference is one `change-outside-writer` span, never typing events. A file with no history, a corrupt one, or another student's opens as a **new attempt**.
- If evidence has been dropped (Word export to RTF/ODT/PDF, certain resaves), show the readable document and label replay unavailable; compare to the school-retained original if permitted.

## Safety

Bound compressed and uncompressed ZIP/XML sizes; reject oversized payloads, path traversal, duplicate/ambiguous relationships, external OPC targets for evidence, XML DTD/external entities and unsafe embedded content; never execute macros. Treat all evidence/comment strings as untrusted and escape on rendering. Explicitly require local-only file processing in teacher viewer.

## Round-trip test matrix

1. InkWitness DOCX -> InkWitness reviewer (must pass).
2. InkWitness DOCX -> Microsoft Word Windows, edit + Save DOCX -> reviewer (release gate; must actually test).
3. Word for Web/OneDrive and Word for Mac (additional school-relevant gates; label untested if not available).
4. LibreOffice edit/save DOCX -> reviewer (verify embedded part and its byte payload).
5. Save as alternate format -> reopen; detect missing history honestly.
6. Add/remove paragraph, change styles, accept/reject normal tracked changes, modify core properties and watermark; verify expected status and no false event sequences.
7. Flip one evidence byte, remove the Custom XML part, alter the final document only, rewrite entire history and recompute hashes; distinguish what local checking detects vs what requires original independent receipt.

**Relevant sources:** Microsoft Open XML packaging docs; Microsoft Word CustomXmlPart API (WordApi 1.4); WordCraft `docs/file-format-parity.md` Custom XML store preservation section. See `docs/SOURCES.md`.


## Public brand versus internal compatibility identifiers

Public application UI, comments and DOCX generator metadata use **InkWitness**. The provisional v1 historical `urn:workalone:*` XML namespace and `workalone:v1:*` event names are **protocol identifiers**, not public branding, and remain unchanged in the starter to avoid accidental schema incompatibility. Migrate to an `urn:inkwitness:*` namespace only with explicit schema-version and import backward-compatibility tests. `docs/BRANDING.md` governs product appearance and upstream trademark restrictions.
