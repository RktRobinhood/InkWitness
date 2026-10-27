<p align="center"><img src="assets/brand/inkwitness-lockup.svg" alt="InkWitness" width="630"></p>
<p align="center"><strong>Every document has a story.</strong></p>
<p align="center"><em>Write · Replay · Understand</em></p>

# InkWitness

**A Word-compatible writing environment with transparent, replayable evidence of how a document developed.**

![InkWitness visual identity](assets/brand/brand-preview.png)

**Brand assets:** [Preview the icons and logo](assets/brand/preview.html) · [Brand usage and upstream replacement map](docs/BRANDING.md). Product routes are **InkWitness Write** and **InkWitness Review**; internal `urn:workalone:*` v1 wire-format identifiers remain intact until an explicitly versioned migration.


> **Project status (27 October 2026): G0 IN PROGRESS — TECHNICAL PREVIEW ONLY. V2 IS A FUTURE ROADMAP ONLY.** WordCraft v0.11.0 is imported in full under `vendor/wordcraft/` (see [`docs/UPSTREAM_LOCK.md`](docs/UPSTREAM_LOCK.md)) and builds to WebAssembly. A rebranded **technical preview** is published to GitHub Pages at `https://rktrobinhood.github.io/InkWitness/` with `/write/` and `/review/` routes. **It does not yet record writing history, export evidence or replay anything.** Every page says so. Do not distribute it to students until the release gates pass.

### Build the site locally

Needs Rust 1.95 with the `wasm32-unknown-unknown` target, and Trunk 0.22.1.

```sh
scripts/build-pages.sh            # writes dist/pages/{index.html,write/,review/,app/,brand/,licenses/}
python3 -m http.server -d dist/pages 8000   # then open http://127.0.0.1:8000/write/
```

For editor development, use upstream's dev server: `cd vendor/wordcraft/apps/wordcraft-web && trunk serve`, then open <http://127.0.0.1:8771/>.

`.github/workflows/pages.yml` builds the same site on every pull request and deploys it to GitHub Pages on every push to `main`.

InkWitness is an open-source, privacy-conscious educational project intended to extend [WordCraft](https://github.com/storytold/wordcraft) (Rust/WebAssembly, MIT OR Apache-2.0). It is being designed for a supervised IB Theory of Knowledge essay writing day, and later for teacher-configurable multi-question assessments. The goal is not to decide whether students cheated, but to preserve a useful, reviewable account of a writing session.

## Project versions — development order

| | **V1 — build and test now** | **V2 — plan for later** |
|---|---|---|
| Hosting | Two static GitHub Pages `/write/` and `/review/` | School-approved Microsoft 365-backed workflow; frontend can remain web-hosted |
| Identity | Required name + assessment title, local attempt ID | School Microsoft Entra ID sign-in and role-based permissions |
| Assignments | Single TOK essay; no remote publishing | Teacher designer/publisher, student assignment dashboard, later multi-stage questions |
| Draft saving | Robust IndexedDB journal + local backups | Same local recovery, plus approved OneDrive/SharePoint sync |
| Hand-in | **One ordinary DOCX**, student submits via existing school channel | Final DOCX delivered through authorized Teams/SharePoint workflow; student keeps a copy |
| Review | Teacher opens DOCX locally at `/review/` | Authenticated teacher inbox/dashboard plus the same DOCX replay |
| Dependency | **No Microsoft registration, tenant permissions, database or custom backend** | Requires school IT/permission feasibility checks before implementation |

**Binding scope rule:** Coding agents must implement **G0–G3 (V1)** first. Do **not** add Microsoft sign-in, Graph dependencies, teacher dashboards, fake cloud buttons, permissions or backend work to V1. V2 development begins only after successful V1 deployment and classroom-readiness gates. Read [Project phases](docs/PROJECT_PHASES.md), [V2 roadmap](docs/V2_MICROSOFT365_ROADMAP.md) and [V2 feasibility checklist](docs/V2_DISCOVERY_AND_SECURITY.md). V2 should reuse V1's interoperable DOCX and replay engine rather than replacing them.

## What the finished V1 project will do

| Student `/write/` | Teacher `/review/` |
|---|---|
| Familiar WordCraft word processor in the browser | Read-only WordCraft rendering of the actual submitted document |
| Required name and assessment title; auto-generated header, properties and optional subtle watermark | Bottom-mounted video-style replay: play, pause, scrub, event pins, speed and skip-idle |
| Clearly disclosed academic-integrity and AI-use policy; confirmation on first deliberate export | Smooth, viewport-aware scrolling; anchored observation cards appear beside the relevant text |
| Local autosave, durable recovery, and optional disk-backed backups | Exact paste/delete/undo/format-event links; optional source-overlap and citation checks |
| **One normal `.docx`** exported with native Word revisions and an embedded InkWitness event log | Import the **same `.docx` locally** and produce a neutral writing-process report |

**One-file hand-in:** The student submits a `.docx` through their school's ordinary assignment system. Teachers can read it in regular Microsoft Word. The optional InkWitness teacher page reads additional replay data stored in an **Open Packaging Conventions (OPC) Custom XML Part** within that same file. This is *not* the discontinued inline custom XML markup feature in Word.

**No application server:** Both pages will be built as static assets and deployed on GitHub Pages. The editor stores drafts in the browser. The reviewer reads DOCX files locally in the teacher's browser. There is no online essay submission database, no silent upload, and no external AI detection API in the baseline. The school still receives the document when the student submits it through the school's established workflow.

## How to start the implementation

1. Create an **empty public GitHub repository**. Upload/extract this starter package into its root and commit it.
2. Open **[`docs/AGENT_PROMPT.md`](docs/AGENT_PROMPT.md)** in Claude Code, Codex or another coding agent. Tell it to read [`AGENTS.md`](AGENTS.md), [`docs/SPEC.md`](docs/SPEC.md), the schema and the test matrix. This is an implementation handoff, not a request for another design proposal.
3. The agent must first carry out **Gate G0** (source build + instrumentation + real DOCX Custom XML + first replay). Follow [`docs/BACKLOG.md`](docs/BACKLOG.md). It should import the **complete WordCraft workspace**, not cherry-pick UI source files; see [`docs/UPSTREAM_PLAN.md`](docs/UPSTREAM_PLAN.md).
4. When G0 works, implement the student app (G1), teacher reviewer (G2), then reliability/privacy/compatibility hardening (G3). **Do not enable the Pages deployment workflow as a functional school tool before the tests pass.**
5. Publish static build outputs at `/write/` and `/review/` using an Actions Pages deployment. The unlisted reviewer address is a convenience, *not* a security control; the reviewer holds no uploaded documents on the site.

Local specification and brand asset checks (no Node, Rust or WordCraft source necessary):

```sh
python3 scripts/validate_starter.py
```

WordCraft's documented upstream WebAssembly development entry point is `cd apps/wordcraft-web && trunk serve`, with the upstream Rust toolchain. Verify the pinned version and source instructions before implementing; they may change.

## Core concepts

- **Observed events:** actual committed editor operations, actual clipboard events where the browser reports them, recovery and import boundaries. A large difference in serialized XML/JSON is *not* a paste event.
- **Advisory findings:** transparent similarity/structure checks and reference leads. None establishes AI authorship, plagiarism or misconduct; never show a purported AI percentage.
- **Recording history:** snapshots and reversible semantic operations with stable event IDs and exact anchors, embedded within the DOCX. The teacher reviewer must never invent missing history.
- **Integrity:** reconstructing event records and comparing the recorded final state with the document detects accidental corruption and many edits. **A locally generated hash inside a student-controlled DOCX cannot authenticate an adversarially modified file**; an independent receipt or controlled hand-in copy is required for independent post-export comparison.
- **Continuation in Word:** students may edit the DOCX later. The previously embedded writing-day history can survive standard Word saves when Custom XML Parts are preserved, but later typing in Word is *not recorded as InkWitness replay*. The teacher should retain the original writing-day submission.
- **Agency and privacy:** the tool visibly tells students what is recorded. It does not fingerprint devices, record raw keystrokes as surveillance, inspect unrelated tabs, read browser-extension lists or send student prose to third-party services.

## Branding and WordCraft attribution

InkWitness is the new public brand. Its original SVG icons, PNGs, favicon and social preview are included in `assets/brand/`; the agent should replace the **application logo, splash/loading identity and favicon**, not every editing command icon. Upstream ArtCraft logos carry trademark restrictions on modified forks: see [the branding contract](docs/BRANDING.md). **InkWitness is independently developed on the WordCraft codebase and is not affiliated with or endorsed by the ArtCraft Team or Microsoft.**

## Project directory

| Path | Purpose |
|---|---|
| `AGENTS.md` | Standing instructions for development agents |
| `assets/brand/` | Original InkWitness SVG logo/icons, favicons, social preview and local gallery |
| `docs/BRANDING.md` | Branding integration, palettes, asset licensing and upstream trademark boundary |
| `docs/SPEC.md` | Product contract and implementation architecture |
| `docs/AGENT_PROMPT.md` | Start-here coding-agent assignment (V1 only) |
| `docs/PROJECT_PHASES.md` | Binding V1 exit gate and V2 scope boundary |
| `docs/V2_MICROSOFT365_ROADMAP.md` | Future teacher/student Microsoft 365 assessment workflow |
| `docs/V2_DISCOVERY_AND_SECURITY.md` | Future school IT, permissions, identity, privacy and feasibility decisions |
| `docs/UPSTREAM_PLAN.md` | What to clone, modify and preserve from WordCraft |
| `docs/DOCX_FORMAT.md` | Standard DOCX parts, evidence envelope and trust semantics |
| `docs/REPLAY_UI.md` | Video-style review behavior and spatial event binding |
| `docs/DIAGNOSTICS.md` | Local-first analysis stack and ethical constraints |
| `docs/STORAGE_AND_RECOVERY.md` | IndexedDB journal, snapshots, backups and failure UI |
| `docs/POLICY_AND_PRIVACY.md` | Student disclosure, academic-integrity declaration, AI-agent notices |
| `docs/BACKLOG.md`, `docs/TEST_MATRIX.md` | Release gates and concrete regression tests |
| `docs/SOURCES.md` | Original vendor/docs/policy research links |
| `schemas/` | Provisional versioned structured data contracts |
| `policies/` | Public policy/AI-assistant declaration examples; not secret controls |
| `examples/` | Fictional example evidence for schema validation only |
| `scripts/` | Bootstrap and starter-validation helpers |

## Constraints and limitations

WordCraft is **alpha software** (upstream estimates incomplete everyday-document readiness). DOCX interoperability, WebAssembly input behavior, editor layout geometry and browser storage must be tested in school browsers. A static page cannot prevent a user from modifying their own browser runtime, cannot reliably identify ChatGPT/Claude/Gemini extensions, and cannot guarantee that local storage survives device loss or site-data deletion. For official examinations, follow school regulations including any separately required supervised monitoring tools; this project does not replace them.

### Academic-integrity policy

The school-based starting reference is [Ikast-Brande Gymnasium's official IB policy page](https://www.ikast-gym.dk/ib/more-about-the-ib/policies). The **Academic Integrity** section currently labels its review date **March 2025**; other sections on the same page have different review dates. Confirm wording with the school before deployment. Student-facing text is a concise paraphrase linked to the policy, not a fabricated quotation.

### Licenses and attribution

Design documents and new project-owned code in this starter are offered under the [MIT license](LICENSE). Importing WordCraft later requires preserving its **MIT OR Apache-2.0** notices, copyright/attribution and original-asset restrictions. Review every third-party library and model license before vendoring it. Do not imply that this educational project is affiliated with or endorsed by WordCraft's authors or Microsoft.

### Security and student records

Public repository, public static JavaScript and public WASM contain **no secrets**. Never commit real student names, essays, activity records, or exported DOCX files to the repo. A hidden `/review/` URL is not access control. See [`docs/POLICY_AND_PRIVACY.md`](docs/POLICY_AND_PRIVACY.md) and [`docs/DOCX_FORMAT.md`](docs/DOCX_FORMAT.md).
