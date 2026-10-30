# Implementation backlog and dependencies

Status convention: `[ ]` not started, `[~]` underway, `[x]` verified. The specification is prepared; none of the application tasks below should be marked complete without actual test evidence.

## G0 - Stop/go technical spike (P0)
- [x] Clone canonical WordCraft and pin verified release/commit; preserve notices. *2026-10-27: v0.11.0 = `e31f2f06a5f8004ceaa88f68c00b47908e321aa4`, full workspace subtree at `vendor/wordcraft/`; LICENSE-MIT/APACHE, NOTICE, ATTRIBUTION kept. See `docs/UPSTREAM_LOCK.md`.*
- [x] Build upstream WASM editor locally and serve it from localhost. *2026-10-27: Rust 1.95.0 + Trunk 0.22.1; `trunk build --release` succeeded (cold 5m41s, 43 MB `-Oz` wasm). `scripts/build-pages.sh` output loaded in headless Chromium 1248 (SwiftShader WebGL2) under `/InkWitness/write/` and `/InkWitness/review/`: editor start screen rendered in about 2 s, zero off-origin requests. Real GPU browsers and school devices: NOT TESTED.*
- [x] Identify exact editable document transaction boundary; add and test event observer. *2026-10-28 (FAL-387): the boundary is the outermost `Session::run`. `crates/engine/src/observe.rs` adds a commit hook there, and the new `crates/inkwitness` `Recorder` turns each committed change into one `evidence-v1` event: block-id anchors in model units, `documentStateDigest` and a provisional `v0` digest chain. Design: `docs/adr/0001-event-anchors.md`. Tests in `crates/inkwitness/src/tests.rs`, all PASS on Rust 1.95 / Linux:*
  - *typing `Knowledge matters` gives 17 `text_insert` events (W01);*
  - *delete, undo and redo (W04);*
  - *a paragraph split or merge is a `structure_change` carrying only the paragraph mark, never a large text event (W05);*
  - *a no-op gives no event; a nested command (`styles.apply` running `para.style`) gives one event;*
  - *a change made outside `run` is recorded as `origin: unknown`;*
  - *New gives `import_document`;*
  - *the event chain verifies.*

  *Real emitted events pass `jsonschema` 4.26 (draft 2020-12) against the event schema. `hostile_params_never_panic` and a recorder-attached variant PASS. Browser UI does not receive events yet (NOT TESTED in the browser). Cross-language (JS) digest vectors: NOT DONE (FAL-388).*
- [x] Correlate actual paste command -> `gestureId` -> committed document operations. *2026-10-28 (FAL-387): `WordApp::run` raises a paste gesture (Ctrl+V, ribbon, context menu), and the session attaches it only to the outermost run of that one paste command.*
  - *Test: a 185-character paste gives exactly one `paste` event, with `gestureId` = `pasteGestureId` = the gesture's id and one op inserting exactly those 185 characters at `b1` offsets 6–191 (W02).*
  - *The gesture never reaches macro recordings or Repeat; a repeated paste has `gestureId: null`.*
  - *UI test `paste_event_links_its_gesture_to_its_commit` feeds a real `egui::Event::Paste` through the app: PASS.*
  - *Clipboard MIME types are recorded when the front end supplies them (engine test). The egui and web paste path exposes text only, so `clipboardTypes` is left out there, not guessed (follow-up proposal in the ADR). `sourceStatus` is always `unknown`.*
  - *A real browser paste: NOT TESTED.*
- [~] Export and reimport Custom XML in a valid DOCX; confirm existing Word revision behavior. *2026-10-28 (FAL-388): WordCraft and LibreOffice round trips PASS. Microsoft Word and Word revision behaviour are NOT TESTED, so this stays open.*
  - *Helper: `wordcraft_docx::{set,find}_custom_xml_part` (`crates/docx/src/custom_xml.rs`). It finds a part by its root element's **namespace**, never by file name. It writes an itemProps with a datastore GUID and `schemaRef`, the `customXml` relationship from `word/document.xml`, and content types. It goes through WordCraft's existing passthrough, so nothing is double-written or orphaned. Setting a namespace again replaces the part in place and keeps its item id.*
  - *Envelope: `crates/inkwitness/src/envelope.rs` writes the `<wa:history xmlns:wa="urn:workalone:evidence:v1">` envelope from `docs/DOCX_FORMAT.md`, with a deterministic gzip+base64 JSON payload. Limits: 32 MiB of envelope XML, 64 MiB of JSON once decompressed. Reading rejects a DOCTYPE, any entity reference, child elements, trailing gzip data and an `attemptId` that differs from the payload's. It follows no relationship (an external `customXml` target is never fetched).*
  - *Tests, all PASS on Rust 1.95 / Linux:*
    - *DOCX write → read gives identical payload bytes;*
    - *two WordCraft open → edit (`text.insert`, `text.newParagraph`) → save cycles keep the part, unchanged and not duplicated;*
    - *two namespaces coexist with a foreign store, and the bibliography keeps `item1.xml` (ours moves to `item2.xml`, or is renamed by the writer if sources are added later);*
    - *duplicate namespace parts are reported as ambiguous;*
    - *every written package has no orphan part and no dangling relationship;*
    - *28 hostile inputs fail cleanly, without a panic: oversize, gzip bomb, bad base64, bad or truncated gzip, wrong namespace, DOCTYPE with an external entity, entity reference, unsupported version, encoding or media type, attempt mismatch, bad UTF-16 and others;*
    - *corrupting any single byte of an envelope never panics.*
  - *Fixture: `vendor/wordcraft/crates/inkwitness/tests/fixtures/g0-evidence.docx` is written by the real WordCraft writer from a real `Session` and `Recorder` (27 typed characters, one paste, a backspace and an undo; 30 events; `Example Student`). Its payload passes `jsonschema` 4.26 against `schemas/evidence-v1.schema.json`. Regenerate it with `cargo run -p wordcraft-inkwitness --example evidence_fixture` from `vendor/wordcraft/`. A test fails if the committed file differs from the writer's output.*
  - *LibreOffice 26.2.6.3, headless `soffice --convert-to docx`: PASS. The part survives; LibreOffice rewrites only the XML declaration and itemProps formatting. `evidence_check` reads identical payload bytes and the same item id. A LibreOffice interactive edit and save: NOT TESTED.*
  - *Microsoft Word (Windows, Mac, web): NOT TESTED. Manual procedure: `docs/WORD_ROUNDTRIP_TEST.md`.*
  - *`cargo xtask ci`, run step by step on Rust 1.95 (2026-10-28): fmt, test (4,203 passed, 0 failed), assets, layers and wasm PASS. clippy FAILS, as it did before this change, on upstream lines Rust 1.95's clippy flags (`doc/math_linear.rs`, `proof`, `xtask`). The crates changed here have no clippy findings.*
- [x] Show actual intermediate rendered state and scroll to exact operation range in WordCraft. *2026-10-28 (FAL-389): `crates/inkwitness/src/replay.rs` rebuilds the document after any `seq`. It starts from a recorder checkpoint and checks every event's `documentStateDigest`. The `/review/` page (`crates/inkwitness-review`) opens a local DOCX (picker or drop, no network) and shows the rebuilt state read-only in WordCraft's renderer. It steps or seeks through events, jumps to the paste, highlights its range, scrolls to it, and shows a side note. Design: `docs/adr/0002-replay.md`; full results: `docs/G0_REPORT.md`.*
  - *Replay tests, all PASS on Rust 1.95 / Linux: every recorded state equals the live session's (typing, paste, delete, undo/redo, font size, bold, heading, margins, bullets, table, merge, cut); deterministic; margins/font/paragraph changes don't move the paste's range; import from its checkpoint; altered or hostile evidence stops cleanly; DOCX round trip.*
  - *View-only mode: `Session::set_read_only` refuses every registered command that could change, replace or save the document (all commands tested), plus a per-frame restore on the review page. Nothing is stored in the document.*
  - *Browser E2E, headless Chromium 1248 (SwiftShader): write → paste 185 characters (real `ClipboardEvent`) → backspace → undo → Ctrl+S download → drop into `/review/` → First event → Jump to paste. PASS: history consistent (86 events), the highlighted text equals the pasted string, the view scrolled from page 1 to the paste on page 2, zero off-origin requests, payload valid against the schema. Screenshots on FAL-389.*
  - *NOT TESTED: Word, school browsers, real GPU browsers, a paste from a real clipboard, the browser file-picker click, IME, long essays, screen readers. Not replayed yet: comments, new stories (headers/notes), settings, media added after the checkpoint.*
- [ ] Microsoft Word Windows DOCX edit/save preservation test (if unavailable, tracked as gate pending).
- [~] Record stop/go decision and upstream gaps with screenshots/fixtures. *2026-10-28 (FAL-389): `docs/G0_REPORT.md` recommends GO for G1 with the Microsoft Word round trip still open; the board decides.*

## G1 - Student writing surface (P0)
- [ ] Mandatory student full name and assessment title, optional class/code, generated attempt UUID; normal DOCX core/custom properties and visible header.
- [ ] Default-on faint name/date watermark with teacher-only privacy override.
- [ ] Visible school academic-honesty policy and truthful process-data notice + supplementary machine-readable AI-agent guidance.
- [ ] Recording restriction in UI; prevent disabling tracking/accept/reject during controlled session.
- [ ] Semantic operation observer for insert/delete/replace/format/structure/undo/redo.
- [ ] Clipboard format metadata with explicit unknown-source semantics.
- [ ] IndexedDB atomic revision journal (target <=500 ms), >=3 validated checkpoint generations, corruption fallback, persistent storage request, quota monitoring, crash restore, per-attempt cross-tab writer lock and transparent save status.
- [ ] First deliberate DOCX Save/Export unchecked honesty checkbox and evidence receipt; **browser autosave remains unconditional**.
- [ ] DOCX exporter with standard Word structure, embedded history and local canonical consistency check; no claim of standalone authentication.
- [ ] One-click finish/download; user-facing instructions to submit via school LMS.

## G2 - Teacher viewer (P0)
- [ ] Static `/review/` local DOCX file picker/drag-drop; no upload.
- [ ] Safe OPC parser and namespace-based Custom XML discovery.
- [ ] Internal chain/endpoint reconstruction verifier and status taxonomy.
- [ ] Read-only WordCraft pages with comment rail and fixed bottom player.
- [ ] Exact event-pin seek and precise before/after range highlighting.
- [ ] Medium smooth auto-scroll and visible-only change reveal; reduced-motion handling.
- [ ] Play/pause/step/seek/timecode/speed/skip-idle/keyboard shortcuts.
- [ ] Review detail, diagnostic summary, neutral interview prompts.
- [ ] Student source file never mutated by teacher; optional annotated export.

FAL-519 demo build (2026-10-29, branch `fal-519-demo-replay`, stacked on FAL-448), toward the items above. Built: the bottom player (Play/Pause, 0.5–16×, Skip idle, Pause at flags, previous/next event and flag, Space/K/←/→/[ ] shortcuts, flag pins with hover cards); flags computed in one place (`inkwitness-review/src/flags.rs`: paste, large paste, large insertion, large deletion, text already there, AI-style phrase quoting the exact chatbot wording, history stops), never a score or an accusation; teacher comments in a layer of their own, drawn on the page as WordCraft comments; Promote to comment with editable, neutral pre-filled drafts; a `(reviewed).docx` review copy with the comments as Word comments and no history; a one-click synthetic sample submission. Tests: `cargo test -p wordcraft-inkwitness-review` (21 pass). NOT TESTED: the review copy in Word desktop; smooth scrolling (FAL-478); comments are in memory only and are lost on reload.

FAL-547 fixes (2026-10-30, branch `fal-547-review-tools`). The timeline and Play start where the writing starts: events at the start that open a document, or leave it as it was (the New the student picks on the start screen, page set-up), take no room and Play starts after them. A New with no text is no longer flagged "Text already there"; one that opened text says how much. Pictures show on the review page and are in the review copy: the history records where a picture goes, not the picture, so the page takes it from the saved file. A picture deleted before the save isn't in the file and still shows as a blank box in the states it was in. Tests: `cargo test -p wordcraft-inkwitness-review` (25 pass), checked on a real writer-page DOCX with a picture. NOT TESTED in a browser.

FAL-547 highlighting and Full ribbon (2026-10-30, branch `fal-547-highlight-ribbon`). Highlight in the review toolbar (yellow, green, blue, pink) highlights the selected text: highlights are teacher marks kept with the comments, anchored in the final document like them, painted over the page without changing the shown document, listed under Comments with Show and Remove, and written into the review copy as Word highlights. New comment in the toolbar opens a draft on the selection. The Full ribbon switch shows WordCraft's ribbon under the toolbar without its File tab or its Share and Editing buttons; the session stays view-only, so editing commands are greyed out until teacher edits (part 4) land. The choice is kept in `localStorage` (`inkwitness.review.fullRibbon`). Tests: `cargo test -p wordcraft-inkwitness-review` (27 pass).

FAL-547 teacher edits (2026-10-30, branch `fal-547-teacher-edits`). With Full ribbon on, the "Your edits" card in the rail takes the teacher's name (kept in `localStorage`, `inkwitness.review.teacherName`). At the last recorded moment the page then becomes an editor whose session tracks changes in that name; the toolbar says "Editing as <name>". The edits are kept in a copy of the final document (`edits.rs`, in memory only) that only the review copy is made from, as Word tracked changes. Stepping back shows the student's text, read-only; the end shows the edits again. Teacher comments and highlights stay anchored in the student's final text and are placed on the edited document through `edits::Map` (a tracked deletion keeps its text, so all the student's characters are still there in order). While editing, the dock's keys move the caret, and the page refuses `file.*`, Track Changes and its options, Accept, Reject, Restrict Editing, Compare, Combine and the document-comment commands. Tests: `cargo test -p wordcraft-inkwitness-review` (30 pass).

## G3 - Reliability and policy (P0/P1)
- [ ] Cross-student identity and cross-tab attempt collision regression tests.
- [ ] Structural changes (paragraph/hard break) never counted as false large text events.
- [ ] DOCX Word Desktop, Word Online, LibreOffice, WordCraft round-trips.
- [ ] Outside-editor continuation clearly separated; missing history degrades safely.
- [ ] Accessible, responsive layout for laptop and Chromebook use.
- [ ] Browser language/IME, keyboard-only and reduced-motion tests.
- [ ] Document importer limits, ZIP bomb and malformed XML protection tests.
- [ ] GDPR/student notice review, retention/deletion guidance and no external calls.
- [ ] Performance check with 2,000-3,000 word essay and thousands of edits.
- [ ] 8/10 student and teacher UX quality gates without blockers.

## Optional after baseline (P2)
- [ ] Teacher-configurable thresholds, source-overlap from supplied local corpus.
- [ ] Light `winkNLP` advisory sentence/repetition module with reviewed model license, no AI score.
- [ ] Local Winnowing/n-gram overlap from explicitly supplied sources; show exact source match and missing-index caveat.
- [ ] DOI/citation syntax check only (online Crossref/OpenAlex later with teacher approval).
- [ ] Teacher note export as separate annotated document/report.
- [ ] Offline cache/service worker after update and storage compatibility QA.
- [ ] Template designer and multi-field assignment model (no UI before MVP).

## Future (out of scope, ADR only)
- [ ] School server receipts and authenticated identity (separate product tier).
- [ ] OneDrive autosave and cross-device sync (school IT approval required).
- [ ] Rich multi-stage InkWitness activities; per-question replay, math grid and timers.


### Branding integration (part of G0/G1, before any public Pages release)

- [x] Inspect `docs/BRANDING.md` and inspect the upstream trademark license. *2026-10-27: upstream `docs/brand/LICENSE-brand.txt` reviewed.*
- [ ] Use existing original InkWitness SVGs/icons and wire product titles, favicon, loading screen and About/Credits.
- [~] Remove protected ArtCraft marks from the distributed modified WordCraft fork; retain source licenses, NOTICE, and upstream attribution. *2026-10-27: 8 ArtCraft mark files deleted, About dialog and titles rebranded, Discord links hidden; remaining strings listed in `docs/UPSTREAM_LOCK.md`.*
- [ ] Verify distinct Write/Review launchers while preserving functional editor toolbar icons.
- [ ] Test GitHub Pages `/inkwitness/write/` and `/inkwitness/review/` path resolution plus small-size icon QA.


## V2 — reserved future backlog, NOT ACTIVE

**Do not start until G0–G3 V1 exit gate is complete and user explicitly authorizes V2.** See `docs/PROJECT_PHASES.md` and `docs/V2_MICROSOFT365_ROADMAP.md`.

- [ ] **V2-0 discovery:** school tenant/IT permissions, MSAL app registration, Graph education assignment eligibility, Teams/SharePoint hand-in path, student-data/retention approval.
- [ ] **V2-1 identity:** Microsoft Entra sign-in, separate teacher/student roles and real assignment authorization; shared-device isolation.
- [ ] **V2-2 teacher designer & publisher:** author, preview, version and distribute assessments to class/students.
- [ ] **V2-3 student dashboard & cloud backup:** assigned-work selection, initialization, local IndexedDB-first recovery plus robust OneDrive/SharePoint syncing/conflict recovery.
- [ ] **V2-4 automatic submission:** final DOCX student copy + separate delivered teacher copy via authorized Teams/SharePoint; verified server hand-in result.
- [ ] **V2-5 teacher dashboard:** received submissions, original evidence-preserving review, feedback/rubric/status.
- [ ] **V2-6 future assignment types:** staged/multi-question questions and configurable policies, preserving V1 replay and DOCX export.

All of the above are **future development items**. No V2 runtime code belongs in the initial V1 proof of concept.
