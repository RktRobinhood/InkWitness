# Agent handoff prompt

**Paste the following into Codex, Claude Code, or another coding agent working in the intended development repository.** Attach or copy the whole package, particularly `docs/SPEC.md`, `schemas/evidence-v1.schema.json`, `docs/TEST_MATRIX.md` and `docs/BACKLOG.md`. This is an instruction to BUILD, not to produce another proposal.

---

You are the lead Rust/WebAssembly and document-format engineer for **InkWitness**. Read all bundled project files before implementation, including root `AGENTS.md` and the new supplemental implementation contracts. Implement **V1 only**. Build a working, tested, **serverless** two-page application, using the official WordCraft source as the base and the original InkWitness brand resources. Read `docs/BRANDING.md` before changing application icons or titles. The critical architecture is:

- `/write/` is a WordCraft-based student DOCX editor with mandatory student name AND assessment title, normal header/metadata and default-on watermark, visible IBG academic-integrity policy and recording notice, first intentional-save checkbox, enforced recording, crash-resistant autosave/recovery, and export of **one normal DOCX** containing embedded InkWitness history.
- `/review/` is a separate WordCraft-based, read-only browser viewer that imports that DOCX locally, plays the actual document from beginning to end, displays anchored diagnostic comments in a side pane, and uses a bottom-fixed video-style transport with exact flag-to-event seeking and comfortable auto-scroll.
- Both routes are static HTTPS pages suitable for GitHub Pages. **No application backend, accounts, API calls, third-party plagiarism service, AI detector API, email submission integration or database server.** Students submit the resulting DOCX in the school's existing assignment system.
- The DOCX must open normally in Microsoft Word; the additional evidence uses a standards-compliant OPC Custom XML Part. Later modifications in Word may preserve previous evidence, but edits made there are **not** retrospectively recorded. Test and label this clearly.
- No AI authorship score, no plagiarism verdict, and no promise of tamper-proof evidence. Use neutral process descriptions and explain browser/clipboard limitations.

## Mandatory version boundary: V1 now, Microsoft 365 V2 later

The existing `/write/` + `/review/` static pages and one portable DOCX are **V1** and MUST be completed, deployed and validated first. `docs/PROJECT_PHASES.md` is binding. `docs/V2_MICROSOFT365_ROADMAP.md` preserves the user's future vision: teacher Microsoft sign-in and assignment design/publishing; student sign-in, assigned-work selection, initialization, editing with local recovery and cloud backup; a student copy and a separate teacher-controlled Teams/SharePoint submission; an authenticated teacher dashboard and replay. **Do not build any of these V2 features, mock them up as implemented, add Graph/MSAL to the V1 dependency graph, or divert G0–G3 effort toward them.** The later integration depends on a school-tenant permissions and GDPR feasibility gate. Preserve versioned event/document IDs so V2 can reuse the V1 core.

## Phase 0: source inspection and feasibility (do this first)

**Brand requirement:** Public product name **InkWitness**. Two routes are **InkWitness Write** and **InkWitness Review**. Original icons are already committed in `assets/brand/`. Replace the upstream browser app title, loading text and favicon and inspect embedded egui branding, without replacing ordinary editing command icons or removing mandatory upstream legal notices. The upstream ArtCraft marks are trademark-restricted and must be removed from modified distributions; see `docs/BRANDING.md`.

1. Check the canonical upstream repository `https://github.com/storytold/wordcraft`. Inspect tag `v0.11.0` and record the actual commit, toolchain, cargo workspace, licensing and build steps from the repository rather than assuming paths/flags.
2. Create a fork-based project structure with upstream notice preservation; do **not** build a new word-processing UI from scratch, or replace WordCraft with an unrelated rich-text editor without documenting a blocking technical reason and requesting approval.
3. Build and run the real WordCraft WebAssembly app locally. Identify exactly where committed document operations, paste commands, native tracked revisions, DOCX export/custom XML and UI layout coordinates can be hooked. Inspect `apps/wordcraft-web/src/host.rs` and `web.rs` among relevant modules. Existing host bridge handles file lifecycle, but does not provide per-edit events.
4. Make a minimal end-to-end technical spike: type characters -> record operations -> export one DOCX with history -> reimport -> reconstruct -> visually seek to edit in the same WordCraft renderer. Test Microsoft Word open/edit/save with a **real** Word desktop environment; if unavailable, label that gate untested and prepare a reproducible manual validation procedure. Do not invent a test result.
5. Verify that the actual student and teacher web outputs display InkWitness identity and favicon, have no upstream trademarked ArtCraft logos, and retain text attribution.
6. Report G0 findings concisely, then proceed through the implementation backlog in order if the design is feasible.

## Implementation rules

- Implement operation/event IDs and `gestureId` links at the editor-model boundary; a paste must reference the exact edit(s). Never classify a serialized paragraph/JSON structural rewrite as a human deletion or paste.
- Separate canonical document state/operation storage from its rendered page coordinates. A seek restores a checkpoint + applies exact operations; smooth scroll finds the target's *current* location in the actual renderer. On offscreen edits, scroll first then reveal the change and the right-side observation. No highlighted offscreen edits, jumping or scroll oscillation.
- Enforce recording and the visible student identity fields *in the InkWitness UI*, while acknowledging a standalone DOCX/browser can be altered by its owner.
- Keep existing standard Word comments/revisions separate from InkWitness observation cards. The teacher viewer can allow notes, but must never silently mutate the submitted source document.
- Implement safe DOCX package parsing and OPC-compliant Custom XML storage. WordCraft file-format notes indicate custom XML preservation, but **test edited resaves on target editors**.
- Use indexed local draft recovery with atomic revision batches, retained checkpoints, saved-through revision status, optional persistent storage, unique attempt IDs, explicit resume/import options, and no silent cross-student history inheritance. Local autosave is unconditional; first manual DOCX Save/Export requires an unchecked academic-honesty declaration.
- On a new/imported essay mark prior text `unobserved import`; capture only subsequent events. Offline paste source/app identification is normally unavailable; do not invent `ChatGPT` origin.
- Store a machine-readable assistant policy only as a companion to a visible student rule and an accurate notice about local editing telemetry. Students must know categories of data being recorded, even if they are not given a technical lecture. Do not frame it as security. Local AI-pattern heuristics are transparent phrase/structure observations only; no AI probability or plagiarism result without evidence.
- For local-first diagnostics select observed WordCraft edits/clipboard as P0, optional Winnowing/n-gram fingerprints for permitted local sources and winkNLP sentence/repetition cues as P1, reference syntax checks as P1, and optional Transformers.js only later. Store named rules/versions and matched excerpts. Never infer clipboard source app or show a calibrated AI probability.
- Use conditional feature flags for optional heuristic scanning and future multi-field assignment config. Do not build future UI prematurely.
- Keep code, npm/cargo lockfiles and licenses, compiled WASM assets in Pages build output, and synthetic test fixtures in the repo. Avoid publishing giant unneeded debug maps or student data. Do not commit real students' documents, tokens or personal data.
- Respect upstream MIT/Apache-2.0 obligations and publish NOTICE/attribution.
- Supply README with local build, GitHub Pages deploy, privacy/trust limitations, normal submission instructions and manual Word interoperability tests. Never claim a security boundary you cannot enforce.

## Quality gates / deliverables

Deliver a repository with: `(0)` fully branded and attributable InkWitness interfaces, using the included editable logo/icon assets; `(1)` working `/write/`; `(2)` working `/review/`; `(3)` real Word-compatible sample DOCX with embedded deterministic fixture history; `(4)` passing automated tests and a test report; `(5)` editable source; `(6)` GitHub Actions Pages deployment instructions; `(7)` Word/LibreOffice round-trip matrix with `tested`, `failed`, or `not tested`; `(8)` short recorded GIF or screenshots of real event-to-flag seek if supported. Do not substitute a mock-up screenshot for a working renderer.

Run `TEST_MATRIX.md` including cross-student attempt isolation, 185-character paste exact seek, hard-break normalization, undo/redo, keyboard navigation, reduced-motion and no history after Word edits. If a required gate fails, state the blocker and most local fix; provide partial functional deliverables and do not mark as completed.

**Working priorities:** reliability of local recovery and real DOCX, exact replay > fancy UI > optional source/AI-style heuristics. Avoid overengineering. A release candidate must earn >=8/10 on both teacher and student experience reviews with no critical blockers.

---

## Guidance for the human operator

Create an empty GitHub repository for this project or let your coding agent prepare the local repository and push it using your own authorized GitHub credentials. The agent must not request passwords in code. Deploy only built app assets to GitHub Pages; students' DOCX files go to the school's established submission process.

Expected first checkpoint from the agent: a **technical spike report**, not a long theoretical discussion, showing the exact pinned source commit, a local browser build, at least one real DOCX round-trip and a working first event observer.
