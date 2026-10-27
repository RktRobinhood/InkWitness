# WordCraft upstream acquisition and modification plan

**Verified research baseline:** 27 October 2026. Upstream canonical repository: https://github.com/storytold/wordcraft. Latest checked release: **v0.11.0** (released 25 October 2026; release lists abbreviated commit `e31f2f0`). The pinned full commit **must be verified by the implementing agent**. Upstream `Cargo.toml` declares Rust **1.95**, edition 2024 and `MIT OR Apache-2.0`. WordCraft remains alpha software.

## 1. Clone or vendor the **full workspace** (not selected UI files)

The upstream Cargo workspace includes `crates/*`, `apps/*`, `xtask`. The DOCX serializer, engine, page layout, renderer, UI and WASM app are coupled through local path dependencies. Copying the `wordcraft-web` directory alone will fail. Preserve original licenses, notices, assets and attribution; do not copy Microsoft iconography.

**Preferred self-contained public repository:** add an upstream subtree to the newly initialized InkWitness repo, from a **clean committed worktree**. Example commands for the coding agent (verify command support first):

```sh
# After you have committed the starter package and confirmed a clean worktree:
git remote add wordcraft-upstream https://github.com/storytold/wordcraft.git
git fetch wordcraft-upstream tag v0.11.0
# Verify full resolved commit, license and tag, then:
git subtree add --prefix=vendor/wordcraft wordcraft-upstream v0.11.0 --squash
# Record git rev-parse v0.11.0^{commit} and the imported subtree commit in docs/UPSTREAM_LOCK.md
```

If a subtree is not practical, the agent may clone the tag and vendor the complete workspace into `vendor/wordcraft/` without nested `.git`, recording source commit and all attribution. Do not silently switch to `main` or an unpinned commit; upstream updates need their own review/testing.

**Alternative for the owner:** fork `storytold/wordcraft` on GitHub and add InkWitness's docs/apps to that fork. Because the user is starting with an empty repository, the subtree route above is the primary plan. A Git submodule that must be edited is *not* recommended without a separate writable fork.


## Mandatory InkWitness branding after import

Read `docs/BRANDING.md` and the upstream `docs/brand/LICENSE-brand.txt`. The ArtCraft name/marks are explicitly **not** covered by the WordCraft software license and must be removed from a publicly distributed modified fork. Preserve the MIT/Apache source attribution, license files and NOTICE.

**Verified source entry points on current main (check pinned v0.11.0):** `apps/wordcraft-web/index.html` has `<title>WordCraft</title>`, a Trunk icon reference to `../../assets/app-icon/wordcraft.ico` and `Loading WordCraft`; `Trunk.toml` uses `public_url = "./"`. Replace the product-facing brand and point built output to `assets/brand/favicon.svg` / `favicon.ico`, using build-safe paths for GitHub project pages. Search egui labels and About/Help panels for identity text. Upstream `assets/app-icon/wordcraft.svg` and native packaging icons are not automatically needed in the web release; inspect original asset licenses rather than blindly deleting files referenced by tests. Do **not** replace the standard ribbon's functional icons.

The InkWitness assets are **already created and packaged** under `assets/brand/`. No image generation or design task is required before G0; use them. The WordCraft editor and viewer are **not yet implemented** in this starter.

## 2. Build before modifying

The upstream README documents:

```sh
cd vendor/wordcraft/apps/wordcraft-web
trunk serve
```

The upstream project also documents `cargo xtask ci` as its integrated test gate. Before running, inspect `AGENTS.md`, `Cargo.toml`, `.cargo/config.toml`, `Trunk.toml`, `rust-toolchain.toml` if present, `xtask` and the current build instructions. Install the pinned `rustup` WASM target and Trunk version as required by that checkout. Do not assume Node-only tooling can build it. Serve `.wasm` with the correct MIME type and configure paths for GitHub Pages' repository prefix.

Initial deliverables: a screenshot of actual editor loaded on localhost, build logs, Rust/toolchain versions, size of emitted assets, and a real edited DOCX. If the toolchain is unavailable, mark G0 not tested rather than claiming a pass.

## 3. Inspect and extend these **candidate** source boundaries

These names are grounded in WordCraft's current public repository; verify on the pinned tag before editing.

| Upstream location | Inspect for | InkWitness modification |
|---|---|---|
| `apps/wordcraft-web/index.html` + `Trunk.toml` | Web title, favicon, loading, asset base URLs | Replace app-facing marks with committed InkWitness icons; preserve correct relative paths in `/write/` and `/review/` |
| `apps/wordcraft-web/src/host.rs` | `?host=parent` same-origin `postMessage` bridge (`ready`, `open`, `save`, `dirty`, `failed`) | Add versioned observation, snapshot, replay/read-only and lifecycle messages with `eventId` and `gestureId`; validate origin/source/attempt |
| `apps/wordcraft-web/src/web.rs` | Web startup, clipboard/image input, browser save/picker | Student/reviewer modes, clipboard metadata and controlled save workflow |
| `crates/engine/` | `Session::run`, commands, undo/redo, state change events | Observe **committed semantic operations**, not keydown-only events or serialized JSON diffs |
| `crates/doc/` | Document model, stable block or node identity | Stable revision anchors and reversible document-model operations |
| `crates/docx/` | OOXML/OPC reader/writer, tracked revisions, comments, properties, watermark, opaque Custom XML | Carry evidence in Custom XML Part, regenerate/update identity metadata and Word comments, preserve standard Word behavior |
| `crates/layout/` and `crates/ui-egui/` | Page geometry, caret/range hit-testing, scroll and margin comment balloons | Read-only replay state, visible-range scrolling, annotation positioning and fixed transport shell |
| `crates/mcp/`, CLI and any control channels | Explicit agent-oriented editing commands | Ensure none are exposed as a student-side writable browser endpoint/bridge; inspect for bypass paths |

WordCraft's `docs/file-format-parity.md` reports preserving opaque Custom XML stores and relations through edited DOCX cycles. This establishes a promising **source-level starting point**, not verified Word desktop/Word Online preservation. Do actual round trips.

## 4. Bounded host interface

Reuse upstream `?host=parent` and add a **versioned host protocol**. Example proposed messages (wire contract to be frozen during G0):

```text
workalone:v1:attempt:init      { attemptId, assignmentId, identity, mode }
workalone:v1:commit            { attemptId, eventId, seq, gestureId, semanticOps, beforeAnchor, afterAnchor, documentStateDigest }
workalone:v1:clipboard:start   { attemptId, gestureId, mimeTypes, observedAt }
workalone:v1:checkpoint        { attemptId, afterSeq, documentModelSnapshot }
workalone:v1:replay:seek       { attemptId, seq }
workalone:v1:replay:geometry   { attemptId, seq, anchor, pageId, rect }
workalone:v1:writer:export     { attemptId, finalEventSeq }
workalone:v1:storage:status    { attemptId, storedThroughSeq, error? }
```

Parent/iframe messages are not authenticated by a shared secret (none exists in a public static site). Check same origin and expected window source, token per in-page instance for accidental crosstalk, schema, size, attempt ID, sequence and state. The instance token is *not* a security guarantee against a user controlling the page. Preserve the original WordCraft `wordcraft:*` behaviors unless deliberately modified with tests.

## 5. Suggested InkWitness-owned architecture after G0

```text
vendor/wordcraft/          # imported whole Rust source, licenses intact
apps/write-shell/          # JS/TS shell for editor iframe and IndexedDB
apps/review-shell/         # JS/TS shell for viewer, replay/report, no uploads
packages/evidence/        # canonical event, hashes, DOCX envelope helpers
packages/diagnostics/     # lightweight browser-local optional analyses
packages/ui/              # shared status and policy components
```

Do not create all these packages immediately; use the smallest viable arrangement while keeping clear code boundaries. Rust-typed event operators may live inside the WordCraft workspace as a crate. Produce a topology ADR with exact choices after G0.

## 6. First technical spike (blocking)

1. Type one paragraph and record events with stable IDs from the real editor model.
2. Paste **185 characters** as one clipboard gesture tied to the exact committed operations.
3. Serialize a standard DOCX with source paragraph and an OPC Custom XML Part containing its event log.
4. Open DOCX in a normal Word-compatible application, verify document text, edits and XML structure, then reimport it.
5. Seek to paste event in *the same WordCraft page renderer* and display its anchored sidebar card; geometry comes from the recovered intermediate model.
6. Verify the actual editor/reviewer WASM builds under a repository-prefixed base path.
7. Record what was **actually tested**; if real Word desktop unavailable, provide a test procedure and label that gate pending.

If G0 fails due to a material WordCraft limitation, do not silently replace it with a simpler editor. Report the specific incompatibility and propose a bounded change to the architecture for approval.
