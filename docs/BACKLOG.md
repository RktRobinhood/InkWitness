# Implementation backlog and dependencies

Status convention: `[ ]` not started, `[~]` underway, `[x]` verified. The specification is prepared; none of the application tasks below should be marked complete without actual test evidence.

## G0 - Stop/go technical spike (P0)
- [x] Clone canonical WordCraft and pin verified release/commit; preserve notices. *2026-10-27: v0.11.0 = `e31f2f06a5f8004ceaa88f68c00b47908e321aa4`, full workspace subtree at `vendor/wordcraft/`; LICENSE-MIT/APACHE, NOTICE, ATTRIBUTION kept. See `docs/UPSTREAM_LOCK.md`.*
- [x] Build upstream WASM editor locally and serve it from localhost. *2026-10-27: Rust 1.95.0 + Trunk 0.22.1; `trunk build --release` succeeded (cold 5m41s, 43 MB `-Oz` wasm). `scripts/build-pages.sh` output loaded in headless Chromium 1248 (SwiftShader WebGL2) under `/InkWitness/write/` and `/InkWitness/review/`: editor start screen rendered in about 2 s, zero off-origin requests. Real GPU browsers and school devices: NOT TESTED.*
- [ ] Identify exact editable document transaction boundary; add and test event observer.
- [ ] Correlate actual paste command -> `gestureId` -> committed document operations.
- [ ] Export and reimport Custom XML in a valid DOCX; confirm existing Word revision behavior.
- [ ] Show actual intermediate rendered state and scroll to exact operation range in WordCraft.
- [ ] Microsoft Word Windows DOCX edit/save preservation test (if unavailable, tracked as gate pending).
- [ ] Record stop/go decision and upstream gaps with screenshots/fixtures.

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
