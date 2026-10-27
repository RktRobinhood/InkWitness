# InkWitness project phases — binding scope contract

**Status:** roadmap/specification, not implemented. **Updated: 27 October 2026.**

InkWitness is **one open-source product with two deliberately separate implementation phases**. These are *not* parallel workstreams. The coding agent MUST complete and stabilize V1 before starting V2 implementation. Read this document before changing project scope.

## V1 — standalone proof of concept (**BUILD NOW**)

- `write/` and `review/` are static GitHub Pages built from a customized WordCraft Rust/WASM workspace.
- Student opens `/write/`, enters required name/assessment information, receives a clear academic-integrity and recording notice, writes in a familiar editor, and gets reliable autosave/recovery in browser storage.
- The only normal hand-in artifact is a standards-compliant **DOCX**, with original Word content, tracked changes where supportable, useful metadata/header/watermark, and an embedded replayable InkWitness event history (OPC Custom XML Part).
- The independent `/review/` page locally opens this same DOCX, plays the real document with time-accurate events and sidebar annotations, and provides neutral process/integrity diagnostics.
- Students may continue in Microsoft Word after the supervised session, but outside edits are NOT represented as witnessed InkWitness events.
- A student submits the DOCX through **existing school procedures** (for example an ordinary Teams assignment). InkWitness V1 does NOT automate this.
- No Microsoft 365 sign-in, app registration, school directory access, roster, automatic publishing/submission, Microsoft Graph calls, remote account sync, database, cloud event receipts, or hosted storage API.
- No claims of AI authorship detection, absolute immutability or secure identity verification. A local hash is a consistency check, not a trusted signature.

**V1 execution order:** G0 real WordCraft/DOCX/replay feasibility -> G1 student writing and recovery -> G2 teacher replay -> G3 interoperability, privacy, recovery and classroom QA -> pilot/stability release.

## Entry gate before V2 development

Only begin coding Microsoft 365 integration after ALL of the following are demonstrated and documented:

1. Real WordCraft WASM student editor works on actual GitHub Pages `/write/` URL.
2. DOCX containing InkWitness history opens normally in Word; the static `/review/` page can reopen it, rebuild all intermediate document states and jump exactly to an inserted passage.
3. Forced recorded editing, a true paste event, correction, undo, formatting, and safe import/recovery all pass automated and manual tests.
4. Local IndexedDB recovery and portable rescue path work on school target browsers. Word and Word-for-web round trip data-loss tests are recorded, including failed/unknown cases.
5. Teacher viewer presents neutral findings, does not misclassify rich-text serialization as paste/deletion, and does not mix two student attempts in a shared browser origin.
6. School/teacher signs off on privacy notice and official policy text; test results and limitations are published. V1 is stable enough for a supervised pilot.

**Before this gate:** V2 appears only in `docs/`, under future roadmap text. No MSAL/Graph build dependencies, permissions, environment secrets, login buttons, mock auth dashboards or unimplemented V2 navigation are to appear in the V1 app.

## V2 — future Microsoft 365-backed assignment lifecycle (**DESIGN ONLY**)

Extend V1 with **Microsoft school sign-in**, teacher assignment authoring/publishing, students seeing assigned work, local-first document editing plus approved OneDrive/SharePoint synchronization, teacher receipt of final submissions via Teams or SharePoint, and a teacher dashboard for replay, comments and assessment. Treat Microsoft 365 as the likely identity and storage/assignment backend; a separate custom server is optional, only if required for trusted receipts or unsupported Graph operations.

The full provisional product journey and the tenant-level feasibility gates are in `docs/V2_MICROSOFT365_ROADMAP.md` and `docs/V2_DISCOVERY_AND_SECURITY.md`.

**Important compatibility commitment:** preserve the V1 DOCX evidence/replay format and teacher local-file import in V2. No teacher or pupil loses ability to open an exported DOCX normally or inspect a permitted offline copy. V2 extends V1; it does not replace its core with a locked-in cloud-only file type.

## How agents should interpret a request to 'implement InkWitness'

Implement V1 only unless an explicit later instruction states that the V1 exit gate passed and V2 implementation has begun. Updating the V2 roadmap is allowed; adding V2 runtime integrations before the gate is not.
