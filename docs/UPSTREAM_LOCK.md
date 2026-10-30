# Upstream lock — WordCraft

| | |
|---|---|
| Upstream URL | https://github.com/storytold/wordcraft |
| Pinned tag | `v0.11.0` |
| Resolved commit | `e31f2f06a5f8004ceaa88f68c00b47908e321aa4` (matches the GitHub tag ref and the abbreviated `e31f2f0` in the release notes) |
| Imported on | 2026-10-27 |
| Import method | `git subtree add --prefix=vendor/wordcraft wordcraft-upstream v0.11.0 --squash` |
| Workspace | Full Cargo workspace: `crates/*`, `apps/*`, `xtask` and all assets |
| License | `MIT OR Apache-2.0` (`LICENSE-MIT`, `LICENSE-APACHE`, `NOTICE`, `ATTRIBUTION.md` kept) |
| Toolchain | Rust `1.95` (workspace `rust-version`, edition 2024), target `wasm32-unknown-unknown`, Trunk `0.22.1` |

## Local changes to the vendored tree

Keep this list current. Every change to `vendor/wordcraft/` must be listed here so upstream updates can be reviewed.

| Path | Change | Why |
|---|---|---|
| `docs/brand/artcraft-*.{png,svg}` | Deleted (8 files). `LICENSE-brand.txt` kept; `docs/brand/README.md` added | `docs/brand/LICENSE-brand.txt` requires modified forks to remove the ArtCraft marks |
| `README.md` | Removed the two ArtCraft logo images (top banner and footer mark) | Same trademark requirement; the plain-text attribution stays |
| `ATTRIBUTION.md` | ArtCraft mark rows now say "Removed from the InkWitness fork" | Keeps the attribution table accurate |
| `apps/wordcraft-web/index.html` | InkWitness title, favicon and loading text; page title and loading text named after the route (`/write/`, `/review/`). The technical-preview banner was removed on board direction (FAL-519, 2026-10-30) | `docs/BRANDING.md` |
| `crates/ui-egui/src/lib.rs`, `chrome.rs`, `backstage.rs`, `ribbon.rs` | Window and title-bar name `InkWitness`; the ArtCraft Discord button, Backstage link and Help-tab button are hidden unless the existing "Show the community button" option is on (now off by default); title test updated | `docs/BRANDING.md` |
| `crates/ui-egui/src/dialogs.rs` | About dialog: InkWitness name, plain-text "Based on WordCraft by the ArtCraft team", no ArtCraft links; copyright line kept | `docs/BRANDING.md`, `LICENSE-brand.txt` |
| `crates/ui-egui/src/i18n/audit.rs` | `InkWitness` allowed as an untranslated product name | Keeps the upstream i18n audit test green |
| `crates/engine/src/observe.rs` | New module: `CommitObserver`, `Commit`, `CommitKind`, `Gesture`, `is_paste` | G0-3/G0-4 commit hook (FAL-387, `docs/adr/0001-event-anchors.md`) |
| `crates/engine/src/lib.rs` | `pub mod observe;` | Same |
| `crates/engine/src/session.rs` | `Session::run` is now a wrapper that reports the outermost run's change to an attached observer; the old body is `run_command`. New fields `observer`, `run_depth`, `gesture` (and in the `edit_snapshot` field list); new `set_observer`, `begin_gesture`, `end_gesture` | Same. With no observer attached the wrapper only counts depth |
| `crates/ui-egui/src/lib.rs` | `WordApp::run` raises a paste `Gesture` around paste commands (old body is `run_asked`); test `paste_event_links_its_gesture_to_its_commit` | G0-4: the gesture is created at the user's paste |
| `crates/ui-egui/src/dialogs.rs` | `cargo fmt` line wrap of the About dialog's version label | That line (from the rebrand) failed `cargo fmt --check` |
| `crates/inkwitness/` | New crate `wordcraft-inkwitness` (layer 5): the evidence `Recorder`, block ids, anchors, digests, tests | InkWitness code kept in its own crate |
| `xtask/src/layers.rs` | `inkwitness` registered at layer 5 | `cargo xtask layers` requires every crate to be classified |
| `Cargo.lock` | `wordcraft-inkwitness` entry | New crate |
| `crates/docx/src/custom_xml.rs` | New module: `set_custom_xml_part`, `set_custom_xml_part_with_id`, `find_custom_xml_part`, `custom_xml_parts`, `remove_custom_xml_part`, `CustomXmlPart`, `CustomXmlError`. It adds, replaces, finds and removes one Custom XML part by the **namespace** of its root element, with an itemProps part (datastore GUID, `schemaRef`), through the existing `customXml.roots` / `embedded.parts` passthrough bookkeeping | G0-5 (FAL-388). Other crates must not hand-write those private manifest keys |
| `crates/docx/src/lib.rs` | `mod custom_xml;` and its `pub use` line | Same |
| `crates/docx/tests/it/custom_xml.rs` | 7 new tests (byte-for-byte round trip, replace in place, edited saves, two namespaces plus a foreign store, bibliography, find by namespace and duplicates, hostile input) and a package-consistency check (no orphan parts or dangling relationships) | Same |
| `crates/inkwitness/src/envelope.rs`, `src/lib.rs` | New `envelope` module: the `<wa:history>` envelope (gzip+base64 JSON, bounded), `encode`/`decode`/`embed`/`extract` | Same |
| `crates/inkwitness/tests/`, `examples/` | Envelope tests, the synthetic fixture `tests/fixtures/g0-evidence.docx` and its builder (`examples/evidence_fixture.rs`), and `examples/evidence_check.rs` | Same |
| `crates/inkwitness/Cargo.toml` | Dependencies `wordcraft-docx`, `quick-xml`, `base64`, `flate2`; dev-dependency `zip` | Same |
| `Cargo.toml` | Workspace dependency `flate2` (`rust_backend`, already in the lock through `zip`) | Same |
| `Cargo.lock` | `wordcraft-inkwitness` dependency list | Same |
| `ATTRIBUTION.md` | Row for `crates/inkwitness/tests/fixtures/g0-evidence.docx` | `cargo xtask assets` requires every committed `.docx` to be attributed |
| `crates/inkwitness/src/replay.rs`, `src/evidence.rs`, `src/tests_replay.rs` | New: the replay engine (checkpoints, per-event application with digest checks, anchor → `Pos`), the evidence-document builder, 9 replay tests | G0-6 (FAL-389, `docs/adr/0002-replay.md`) |
| `crates/inkwitness/src/lib.rs`, `src/blocks.rs` | The recorder writes checkpoints (on attach and after each import), `setPart` ops for page setup/styles/numbering, `storyBlocks` when a table comes or goes, and leaves typing's `content` out only when replay's text rule rebuilds it exactly | Same |
| `crates/inkwitness/src/envelope.rs` | `gunzip_bounded` shared with checkpoint reading | Same |
| `crates/inkwitness/tests/common/fixture.rs`, `tests/fixtures/g0-evidence.docx` | The fixture's evidence now has its checkpoint; fixture regenerated | Same |
| `crates/inkwitness/Cargo.toml` | Dev-dependency `wordcraft-layout` (a test lays the replayed page out) | Same |
| `crates/inkwitness-review/` | New crate `wordcraft-inkwitness-review` (layer 7): the writer hook (records edits, embeds the history on Save), the `/review/` page (load, verify, step/seek, highlight, side note, live status region), `Route::from_url`, the browser `Env` | Same. InkWitness UI code kept in its own crate |
| `xtask/src/layers.rs` | `inkwitness-review` registered at layer 7 | Same |
| `crates/engine/src/session.rs` | `Session::set_read_only`, `read_only`, `refuses_read_only`; `run` refuses those commands on a view-only session and puts back any change another command made; field `read_only` (also in the `edit_snapshot` field list) | Same: the review page's explicit read-only mode, stored nowhere in the document |
| `crates/engine/src/observe.rs` | Tests: no registered command changes, replaces or saves a view-only document | Same |
| `crates/ui-egui/src/canvas.rs` | `CanvasState::review_mark` and its amber highlight, drawn like Read Aloud's | Same |
| `crates/ui-egui/src/lib.rs` | `Services::prepare_save` (`PrepareSaveFn`): the document the web Save writes; `WordApp::execute` refuses a view-only session's save/edit commands before the web save path | Same |
| `apps/wordcraft-web/src/web.rs`, `Cargo.toml` | Route from the URL path (`/review/`) or `?mode=review` (parsed, not searched); Review: view-only start page, no unload guard, files from the picker and drops go to the review, status region; Write: recorder attached, Save embeds the history | Same |
| `Cargo.lock` | `wordcraft-inkwitness-review` entries | New crate |
| `crates/ui-egui/src/theme.rs` | `Palette` (`WordCraft`, `Critic`), `Tokens::critic_light`/`critic_dark`/`of`, `theme::critic` (brand colours, `Accents`, `Tint`), `apply_palette` (`apply` now calls it with `WordCraft`); `Tokens::get` returns the installed palette's tokens; the contrast tests run over all four palettes and a new `critic_palette_meets_wcag_aa` | FAL-448: the review page's Critic interface theme (`docs/BRANDING.md`). WordCraft's own colours are unchanged |
| `crates/ui-egui/src/lib.rs` | `WordApp::viewer_chrome` and `WordApp::palette` (both per page, never saved); `applied_theme` keyed by palette; `fonts_ready`; with `viewer_chrome` the frame draws only the panes and the page (no title bar, ribbon, status bar, File page or keytips), and `execute` refuses the `ui.*` commands that open editing UI (`viewer_allows`) | FAL-448: viewer-only `/review/`. Off by default, so `/write/` and the desktop app are unchanged |
| `crates/ui-egui/src/panes.rs` | With `viewer_chrome`, only the Navigation and Comments panes, and the Comments pane without New Comment, Reply, Resolve or Delete | Same |
| `crates/ui-egui/src/canvas.rs` | With `viewer_chrome`, no context menu and no Reply/Resolve/Delete on the active comment balloon | Same |
| `crates/ui-egui/src/canvas.rs` | With `viewer_chrome`, no rulers (their indent, margin and tab-stop markers and the tab-stop selector edit the page), whatever `view.ruler` says | Same |
| `crates/ui-egui/src/mini_toolbar.rs` | No mini toolbar with `viewer_chrome` | Same |
| `crates/ui-egui/src/widgets.rs` | `enabled` is false for a command a view-only session refuses | Same: refused controls show greyed out |
| `crates/inkwitness/src/replay.rs` | `State::unrecorded` | FAL-448: a file with no history is shown as saved ("History missing") instead of refused |
| `crates/inkwitness-review/` | New `toolbar.rs` (the viewer's toolbar, `SHOWN`/`HIDDEN`, `setup`), `rail.rs` (the "Observations & notes" rail) and `card.rs` (the reusable rail card); `review.rs` draws them (`Review::chrome` replaces `panel`); tests for the hidden controls, viewer commands and the paste card; dependency `chrono`, dev-dependency `egui_kittest` (both already in the workspace) | FAL-448 |
| `apps/wordcraft-web/src/web.rs` | Review: `toolbar::setup` (viewer chrome, Critic palette, page width), `Review::chrome`, and no preference writes from the review page | FAL-448 |
| `Cargo.lock` | `wordcraft-inkwitness-review` dependency list | Same |
| `crates/ui-egui/src/canvas.rs` | `CanvasState::review_highlights`: the review page's highlights, painted over their ranges (the document isn't changed) | FAL-547: teacher highlights |
| `crates/ui-egui/src/lib.rs` | `WordApp::viewer_ribbon`: with `viewer_chrome`, the ribbon is drawn above the panes; `viewer_allows` takes whether it is shown and then lets `ui.tab` switch tabs (never to File) | FAL-547: the review page's Full ribbon switch. Off by default |
| `crates/ui-egui/src/ribbon.rs` | With `viewer_chrome`, no File tab and no Share, Editing or Comments button in the tab strip | Same |
| `crates/ui-egui/src/widgets.rs` | `enabled`: what a viewer refuses (`viewer_allows`) shows greyed out | Same |

Not yet rebranded (tracked as a follow-up): the Help-tab keytip and the `ui.discord` command, Options and default-app strings, `web.rs` start-up error text, the sample document link, and generator metadata in `crates/docx` (`Application`), `crates/pdf` and `crates/formats/src/latex.rs`.

## Updating upstream

Do not move to `main` or another tag without review. To take a new release:

```sh
git fetch wordcraft-upstream "refs/tags/<tag>:refs/tags/<tag>"
git rev-parse "<tag>^{commit}"            # record it above
git subtree pull --prefix=vendor/wordcraft wordcraft-upstream <tag> --squash
```

Then re-check the brand-mark removal, rebuild, and rerun the test matrix.
