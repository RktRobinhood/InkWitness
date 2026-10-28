# G0 report — WordCraft, DOCX evidence and replay feasibility

- Date: 2026-10-28
- Covers: backlog G0-1 to G0-6 (FAL-384 to FAL-389)
- Upstream: WordCraft v0.11.0 (`e31f2f06`), vendored at `vendor/wordcraft/`
- Toolchain: Rust 1.95.0, `wasm32-unknown-unknown`, Trunk 0.22.1, Linux
- Browser: headless Chromium 1248 with SwiftShader WebGL2

## Recommendation for G1

**GO, with one G0 gate still open: Microsoft Word.** The board decides.

The technical risks G0 was meant to retire are retired in the real WordCraft code. Nothing was mocked:

- the editor runs in the browser;
- every committed edit is observed;
- a paste links to the exact operations it made;
- the history travels inside a real DOCX;
- `/review/` rebuilds any recorded state in WordCraft's own page renderer and seeks to the paste.

**What is not proven is whether Microsoft Word keeps the evidence part when a student opens and saves the DOCX.** It has not been tested on any Word (Windows, Mac or web). LibreOffice keeps it. The manual procedure is in `docs/WORD_ROUNDTRIP_TEST.md` and takes about ten minutes on a machine with Word.

G1 work does not depend on that answer, because the student writes and saves in InkWitness, not Word. The answer decides what G2 must say about files that went through Word. If Word drops the part, `/review/` already reports that as "History missing" rather than guessing. So I recommend starting G1 now and running the Word test in parallel, before G1 is signed off.

## What works, with the commands

Commands run from `vendor/wordcraft/` unless stated otherwise. The environment comes from the task's toolchain block (`RUSTUP_HOME`, `CARGO_HOME`, `PATH`).

| G0 item | Result | Evidence and command |
|---|---|---|
| G0-1 Pinned upstream, notices kept | PASS | `docs/UPSTREAM_LOCK.md` |
| G0-2 WASM build, served locally | PASS | `env -u BASH_ENV scripts/build-pages.sh _scratch/pages` (repo root): 44 MB wasm. Live preview: https://rktrobinhood.github.io/InkWitness/ |
| G0-3 Commit-boundary observer | PASS | `cargo test -p wordcraft-inkwitness` (recorder tests); `docs/adr/0001-event-anchors.md` |
| G0-4 Paste → `gestureId` → exact operations | PASS (native and browser) | Recorder test W02. In the browser: a real `ClipboardEvent('paste')` gave one `paste` event with a `gestureId` and exactly 185 inserted characters (E2E below) |
| G0-5 Evidence Custom XML part in DOCX | PASS in WordCraft and LibreOffice; **Word NOT TESTED** | `cargo test -p wordcraft-inkwitness --test envelope`, `cargo test -p wordcraft-docx --test it custom_xml`; `docs/WORD_ROUNDTRIP_TEST.md` |
| G0-6 Rebuild an intermediate state, seek and scroll to the paste in the real renderer | PASS | Replay engine and tests: `cargo test -p wordcraft-inkwitness replay`. App path: `cargo test -p wordcraft-inkwitness-review`. Browser E2E below. Design: `docs/adr/0002-replay.md` |

### G0-6 in detail

**Replay engine** (`crates/inkwitness/src/replay.rs`). It rebuilds the document after any `seq`:

- it starts from a checkpoint, written by the recorder on attach and after each import;
- it applies each event's ops by block id;
- after every event it checks the event's `documentStateDigest`, and at the end the export's `recordedStateDigest`.

Tests, all PASS:

- **`replaying_every_event_reproduces_every_recorded_state`.** A live session is compared to its replay at every `seq`: body, parts, page setup, styles and numbering. The session covers typing, Enter, a 185-character paste, delete, undo, redo, a font size over the whole document, bold, Heading 1, a Narrow margins preset, a bulleted list, a 2×2 table, a paragraph merge and a cut.
- `replay_is_deterministic`: two identical sessions give the same events and the same states.
- **`layout_changes_dont_move_the_flag`.** Wide margins and a 20 pt font are set before the paste, and a paragraph is inserted above it afterwards. Seeking still marks exactly the pasted text, on the replayed page with the recorded margins. Widening the reviewer's margins by 2 inches re-wraps the same model range onto more lines, and the text is unchanged.
- `an_import_replays_from_its_own_checkpoint`; without that checkpoint, replay reports `NoCheckpoint` instead of guessing.
- `altered_evidence_stops_the_replay`: changed text, changed content, a missing block, a wrong digest, a numbering gap or a corrupt checkpoint each stops the replay at that event.
- `hostile_evidence_never_panics`: 7 junk values in each of 13 fields of every event.
- `the_docx_carries_a_replayable_history`: DOCX write → read → extract → replay → final digest matches.

**Review page** (`crates/inkwitness-review`, `/review/` route):

- it opens a local DOCX from the file picker or a drop, with no network;
- it shows the rebuilt state in WordCraft's own renderer, read-only;
- it has First, Previous, Next, Last, an event slider and "Jump to paste";
- the paste range is highlighted in amber and the caret moves to it, so the page scrolls it into view;
- a side note shows type, origin, time, inserted/deleted length, `gestureId`, clipboard formats, source "unknown", and whether the anchor's context hash matched;
- it labels the file's history as consistent, externally changed, corrupt or missing.

**Read-only.** The engine's new `Session::set_read_only` refuses every mutating command, plus Undo, Redo, Repeat, New, Open, Close, Save, Save As, Rename, Encrypt and Recover. A change any other command makes is put back. Nothing is stored in the document; Restrict Editing is not used. Tests:

- `a_view_only_session_refuses_every_change` runs all registered commands against a view-only session;
- `the_reviewed_document_cannot_be_changed` tries typing, paste, backspace, undo, margins, New and Save through the real `WordApp`.

### End-to-end browser test

Script: `_scratch/browser/e2e-389.js` (not committed; `_scratch/` is ignored). It drives the production build at `http://127.0.0.1:8389/InkWitness/`:

1. `/write/`: File › Home › Blank document.
2. Type `Opening line.` and 40 Enters, so the paste lands on page 2.
3. Type `Example Student wrote this. `.
4. Paste 185 characters as a real `ClipboardEvent` with `text/plain`.
5. Backspace, then Ctrl+Z.
6. Ctrl+S. The save picker is disabled, so the DOCX arrives as an ordinary download.
7. `/review/`: drop the downloaded DOCX on the page.
8. Click First event (the empty page, at the top), then "Jump to paste".

Result (2026-10-28): **PASS.**

- 86 events, history "Consistent".
- The shown event is `seq` 84, `type: paste`, with a `gestureId`.
- **The highlighted range's text equals the 185-character pasted string exactly.** It is read from the rebuilt model through the page's status region, not from the event's `insertedText`.
- The view scrolled from the top of page 1 to the paste on page 2: 22,460 highlight-coloured pixels after the seek, against 412 before (the banner and ribbon icons).
- Zero off-origin requests.
- The browser-written payload passes `jsonschema` 4.26 against `schemas/evidence-v1.schema.json`.

Screenshots are attached to FAL-389:

- `03-write-undone.png`: the writer after undo;
- `05-review-opened.png`: review at the last event;
- `06a-review-first-event.png`: the first event, at the top;
- `06-review-paste.png`: the paste highlighted on page 2 with its side note.

## What was tested on which browser

| Environment | Tested |
|---|---|
| Native Linux (Rust 1.95, `cargo test`) | All the unit and app-level tests above |
| Headless Chromium 1248, SwiftShader WebGL2, Linux | The E2E above; the G0-2 smoke test of `/`, `/write/` and `/review/` |
| LibreOffice 26.2.6.3, headless convert | The evidence part survives a DOCX → DOCX conversion (G0-5) |

## NOT TESTED

- **Microsoft Word for Windows, Mac and the web.** Whether Word opens the DOCX normally, keeps the evidence part on save, and how its revisions interact (`docs/WORD_ROUNDTRIP_TEST.md`).
- **School browsers and devices:** Chromebooks, managed Edge or Chrome, Safari, Firefox, iPad. Also any real GPU browser; only SwiftShader in headless Chromium was used.
- **A paste from a real clipboard.** The E2E dispatches a synthetic `ClipboardEvent`. A paste from another application, and the clipboard formats a real browser exposes, are untested. eframe passes text only, so `clipboardTypes` is absent in the browser.
- **The file picker in the browser.** The E2E uses drag-and-drop. The "Open DOCX…" button uses the same inbox path as WordCraft's own Open, but that click was not driven.
- IME and mobile keyboards (W09).
- Track Changes during a recorded session.
- Long sessions (2,000–3,000 words, thousands of events): speed and memory.
- Reduced motion, keyboard-only use and screen readers on the review page. The page has a live status region, but nobody has tried it with a screen reader.

## Upstream gaps and known limits

- **Replay does not cover everything.** Not replayed: comments; a new story such as a header, footer or note created after the checkpoint (reported as `Unsupported`); document settings; and media bytes added after the checkpoint (pictures). The state digest (`v0`) covers the stories only. See `docs/adr/0002-replay.md`.
- **Scrolling uses WordCraft's caret-follow.** That is a 40–60 px margin, not the 15–20% comfort margin or the eased motion `docs/REPLAY_UI.md` asks for. The paste can sit at the viewport's bottom edge (as in `06-review-paste.png`). The smooth, bounded scroll is G2.
- **The writer has no identity or attestation yet.** The G0 writer puts "Not entered (G0 technical preview)" and policy `g0-preview-no-attestation` in the evidence. Onboarding and the first-export attestation are G1.
- **Every paste in the browser, and every other non-typing command, carries its paragraph model.** The 86-event E2E file has a 133 KB payload; most of it is the paste event's `content` and the checkpoint. G1 should measure a real essay and add periodic checkpoints.
- **The desktop app's Save does not embed the history.** Only the web Save path calls `Services::prepare_save`. The desktop path writes through the engine's `file.save` command, which has no hook.
- **`cargo xtask ci` clippy.** It fails on upstream lines that Rust 1.95's clippy flags (`doc/math_linear.rs:559`, `proof/affix.rs:313`, `engine/cmd/text.rs:796` and others). These were reported in PRs #2 and #3 and are untouched here. The crates this PR adds or changes are clean under `-D warnings`.
- **Upstream changes.** All of them are listed in `docs/UPSTREAM_LOCK.md`. The vendored-crate edits for G0-6 are a read-only flag in `Session`, one canvas highlight, and one save hook in `ui-egui`.

## `cargo xtask ci` (2026-10-28, this branch)

`cargo xtask ci` stops at its first failure, which is clippy. The steps after it were run one by one (`cargo xtask <step>`).

| Step | Result |
|---|---|
| fmt | PASS |
| clippy | **FAIL**, on upstream lines only: `doc/math_linear.rs:559` stops the run, and `engine/cmd/text.rs:796` is another. Under `-D warnings` with `--no-deps`, `wordcraft-inkwitness`, `wordcraft-inkwitness-review` and `wordcraft-web` (native and wasm32) have no findings. `wordcraft-engine` and `wordcraft-ui-egui` have none besides upstream `nonminimal_bool` findings |
| test | PASS: 4,219 passed, 0 failed, 4 ignored (+16 new tests over PR #3's 4,203) |
| i18n-report, doctest | PASS |
| assets | PASS |
| layers | PASS: 22 crates, no violations; `inkwitness-review` at L7 |
| wasm | PASS. The step's fixed crate list doesn't include the new review crate; `scripts/build-pages.sh` compiles it for wasm32 in the release build (PASS) |

Not run on this machine: nothing. Every step ran.
