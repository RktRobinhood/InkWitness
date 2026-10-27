# Instructions for coding agents — InkWitness

This is a **development-agent** instruction file. It is not the assessment policy that visiting student-facing AI assistants are guaranteed to see. See `policies/ai-assistant-guidance.json` for the latter.

## Mission

Build **V1 only**: the real, tested, two-route, static-hostable WordCraft fork described in `docs/SPEC.md`. The V2 Microsoft 365 platform is a **future roadmap**, not a simultaneous implementation. Preserve normal DOCX interoperability, faithful edit provenance and student's ability to recover work above visual polish. This is a **coding assignment**, not another research memo or a redesigned word processor.

## Read first, in order

1. `README.md` and `docs/PROJECT_PHASES.md`
2. `docs/BRANDING.md`
3. `docs/AGENT_PROMPT.md`
4. `docs/SPEC.md`
5. `docs/UPSTREAM_PLAN.md`
6. `docs/DOCX_FORMAT.md`, `docs/REPLAY_UI.md`, `docs/STORAGE_AND_RECOVERY.md`
7. `docs/DIAGNOSTICS.md`, `docs/POLICY_AND_PRIVACY.md`
8. `schemas/*.json`, `docs/BACKLOG.md`, `docs/TEST_MATRIX.md`

## Work plan and reporting

- **V2 SCOPE LOCK:** Do not implement Entra/MSAL, Graph, OneDrive/SharePoint sync, Teams integration, teacher assignment designer, school roster, or authenticated dashboards until the V1 exit criteria in `docs/PROJECT_PHASES.md` have been verified and the user explicitly starts V2. V2 planning documents are provided to protect architecture and requirements, not to create parallel tasks.
- **G0 (stop/go)**: verify source commit and license; import the complete WordCraft workspace; build WASM; instrument semantic edit + clipboard; embed and reread genuine Custom XML from actual DOCX; replay a real intermediate WordCraft document state. Report precise status and blockers with reproducible commands. Do not claim Word desktop testing unless actually performed.
- **G1**: student onboarding, identity/header/metadata/watermark, academic-honesty notice + first manual export attestation, autosave, events and single DOCX.
- **G2**: teacher local DOCX import, read-only WordCraft page rendering, bottom video transport, smooth visible-page auto-scroll, exact diagnostic pins, side comments and report.
- **G3**: cross-browser and Word interoperability, crash recovery, accessibility, privacy, hostile DOCX and performance.
- Keep `docs/BACKLOG.md` updated with real results, NOT TESTED labels and test artifacts. Finish each stage with working source and commands.

## Hard requirements

- Build and present the app publicly under the **InkWitness** identity using `assets/brand/` and `docs/BRANDING.md`. Replace only upstream product marks, not functional formatting icons. Remove ArtCraft trademarked marks from distributed modified forks; preserve source attribution and original code licenses.
- Use the official `storytold/wordcraft` source as the base. Import the full Rust workspace; do not recreate its renderer or replace it with a fake HTML `contenteditable` mock.
- Respect upstream `AGENTS.md`, LICENSE-MIT/LICENSE-APACHE or equivalent, `ATTRIBUTION.md`, and no-unsafe/no-panic source rules. Check exact upstream filenames on the pinned tag.
- The browser writer must observe committed document operations. A `paste` flag must link through a durable `gestureId` to the **exact committed operation(s)** with stable before/after anchors. Do not infer paste from `string.length` or use nearest timestamp.
- A playback checkpoint is model state, not captured screen video. Restore and replay model operations in WordCraft's page renderer. Changing margins/font/layout must not send flags to the wrong paragraph.
- The teacher tool is a local-file reader. No student record uploads, remote LLM calls, analytics, third-party script/CDN or student-data uploads by default.
- New/import/resume options **must isolate attempt identity**. Reproduce previous same-browser history leakage as a regression test.
- All input metrics and source overlap are advisory. **Do not assign an AI-authorship probability or declare misconduct.** Source links from clipboard HTML are unverifiable hints; a browser cannot identify the source application reliably.
- Student-facing rules must disclose the categories of observations; open-source source code cannot contain hidden 'anti-cheat secrets'. Do not bypass accessibility tools or require restrictive surveillance extensions.
- A hash inside a DOCX is not a signature. Separate `local consistency`, `externally modified/unobserved`, `corrupt`, and `independently receipted` in the reviewer. Never label self-contained local hashing `tamper-proof`.
- Keep normal manual DOCX saving blocked until a first-save checkbox is checked; **never block automatic browser recovery** while waiting for an attestation.
- Document and test technical assumptions. Do not describe unverified features as built or production-ready.

## Brand integration gate

After importing the pinned WordCraft source, locate and replace the app title, favicon and loading identity in `apps/wordcraft-web/index.html` and any product-facing logos in the generated Write and Review shells. Explicitly review `docs/brand/LICENSE-brand.txt` in upstream; remove protected ArtCraft brand assets from the distributed fork and preserve all legal notices. Check both routes for correct favicons and a readable app name; there is **no working app in this spec starter**. Do not replace established editing-command icons with custom brand art.

## Hard security/privacy boundaries

- **V1 only:** public GitHub Pages build, no backend, identity provider, shared secrets, external credential storage or user-upload endpoints. Future V2 may integrate Microsoft 365 after separate school approval; this does not change the V1 boundary.
- Do not commit actual pupil data, clipboard text from real sessions, course submissions, or browser logs. Use synthetic names (`Example Student`) in examples.
- Limit DOCX/ZIP size and expansion, disallow external relationship fetching, sanitize imports and ensure XML parsing does not load remote DTD/entities.
- Treat offline/externally edited intervals honestly. Do not infer activity during those periods or claim Word's version history is embedded in an emailed DOCX.

## Publishing

The `.github/workflows` folder initially validates **specification files only**. Do not enable Pages deploy until the real WASM build creates `/write/index.html` and `/review/index.html`, the required test gates pass, and README contains correct usage instructions. No placeholder student editor may be presented as working.

## Definition of done

A student can write, paste a known 185-character string, delete, undo, save a genuine DOCX, then a teacher can open that **same DOCX** in Word and in `/review/`. The viewer shows a real WordCraft rendered document, seeks exactly to the paste, displays its right-rail annotation, smoothly scrolls into view and reconstructs the final recorded document. Crash/reload recovery and same-browser student isolation pass. Record every untested editor/platform explicitly.
