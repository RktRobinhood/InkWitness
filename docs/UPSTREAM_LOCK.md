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
| `apps/wordcraft-web/index.html` | InkWitness title, favicon and loading text; technical-preview banner; route name for `/write/` and `/review/` | `docs/BRANDING.md` |
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

Not yet rebranded (tracked as a follow-up): the Help-tab keytip and the `ui.discord` command, Options and default-app strings, `web.rs` start-up error text, the sample document link, and generator metadata in `crates/docx` (`Application`), `crates/pdf` and `crates/formats/src/latex.rs`.

## Updating upstream

Do not move to `main` or another tag without review. To take a new release:

```sh
git fetch wordcraft-upstream "refs/tags/<tag>:refs/tags/<tag>"
git rev-parse "<tag>^{commit}"            # record it above
git subtree pull --prefix=vendor/wordcraft wordcraft-upstream <tag> --squash
```

Then re-check the brand-mark removal, rebuild, and rerun the test matrix.
