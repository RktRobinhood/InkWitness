# Teacher replay UX — required behaviors

**Design principle:** A teacher watches *the original WordCraft-rendered document develop*, not a video of a separate text pane. Replay is read-only. The document remains the focus, diagnostic comments occupy a right-hand rail, and playback controls form a **fixed bottom dock like a video player**.

## Board direction (FAL-446, 2026-10-28)

The board set four rules for `/review/`. Status is as of FAL-448 (T1); T2 and T3 are **NOT BUILT** yet.

### Viewer-only chrome (built in FAL-448)

- `/review/` shows **no control that edits the document**. WordCraft's title bar, ribbon tabs, quick-access bar, status bar, File page, keytips, context menu and mini toolbar are not drawn there. Nor are AutoSave, Share, the Editing-mode switch, Dictate, Editor, or any save path.
- What the page keeps, in its own navy toolbar: **Comments** (show or hide, previous, next), **View** (zoom out, zoom level, zoom in, one page, page width, dark page), **Find** (the Navigation pane), and **Open DOCX…**. The Comments pane lists the file's comments without New Comment, Reply, Resolve or Delete.
- Two independent locks:
  - the session is view-only (`Session::set_read_only`), so the engine refuses every editing command;
  - the front end is a viewer (`WordApp::viewer_chrome`), so keyboard shortcuts can't open editing UI. Of WordCraft's `ui.*` commands, only the Zoom and Word Count dialogs and Open run.
- `crates/inkwitness-review/src/toolbar.rs` lists the shown controls (`SHOWN`) and the hidden ones (`HIDDEN`). A test walks the page's accessibility tree and fails if any control other than a viewing control appears.
- The review page reads the writer's stored preferences but never stores its own.
- `/write/` is unchanged.

### The rail's three kinds of item

| Kind | Where it comes from | Who sees it | In the copy returned to the student? | Status |
|---|---|---|---|---|
| Word comment | Already in the DOCX | Everyone who opens the file | Already there, unchanged | Shown in the Comments pane (FAL-448) |
| Observation | InkWitness evidence: paste, imported document (later: large insertion, recovery gap and the rest of the taxonomy) | **Teacher only** | Never, unless promoted | Paste and import cards (FAL-448) |
| Teacher comment | Written by the teacher, or promoted from an observation or event | Teacher, and the student once the review copy is returned | Yes, as a standard Word comment | **NOT BUILT** (T2) |

Each kind has its own icon, words and colour: Review Gold for observations, Witness Blue for teacher comments and actions, Slate for Word comments and ordinary events. Colour is never the only sign. One card component (`crates/inkwitness-review/src/card.rs`) draws every kind, and later the timeline's hover cards. It has:

- a type chip with an icon and words;
- the recorded time and the time elapsed in the session;
- the size in words and characters;
- the excerpt;
- a Go to action;
- `gestureId`, the anchor check, the clipboard formats and the source, folded under **Details**.

The rail ("Observations & notes") has three parts:

- a header with the InkWitness Review mark, the file name, the student and the assessment (or "not entered"), and a history badge (Consistent, External changes, Corrupt or Missing, each with an icon and words), followed by the quiet line "Unsigned local history: not proof of authorship.";
- the event navigation, which FAL-477 moves to the bottom dock;
- the card for the moment shown, then one card for each observation;
- when the file's text differs from its history's last state, a final section "Changed outside InkWitness (not observed)" (FAL-664, [ADR 0005](adr/0005-changes-outside.md)): the file opens on it, the page shows the file as it stands with the added text marked, and the rail lists what was added and removed, with flags like a paste for large additions and a Promote to comment for each change. It is not an event and says the timing and source are unknown.

### Promotion (NOT BUILT, T3)

- Every observation card and timeline event gets a **Promote to comment** action. The action opens an editable draft, pre-filled from the evidence with:
  - what happened, when, and how much;
  - the excerpt;
  - one neutral interview question.

  The draft never mentions AI or misconduct and leaves out internal values (`gestureId`, hashes) unless the teacher adds them.
- Nothing reaches the student until the teacher saves the draft. A promoted observation and its comment link to each other.
- Teacher comments live in a review layer, never in the student's document model or evidence. Export writes `<original name> (reviewed).docx`. The original file is never written, and nothing is saved automatically.

### The Critic theme (built in FAL-448)

- The theme is an interface palette (`theme::Palette::Critic` in `crates/ui-egui/src/theme.rs`), set per page. It is not a saved preference. Light or dark still follows the Appearance setting.
- `/review/` uses Critic. `/write/` keeps WordCraft's own palette.
- The colours come from `docs/BRANDING.md`:
  - Ink Navy `#13213A` for the header and text;
  - Paper Blue `#EAF3FF` for light surfaces;
  - Review Gold `#EDBA69` for observations;
  - Witness Blue `#3685F5` for teacher actions;
  - Slate `#60728B` for marks.
- Where a brand colour fails WCAG AA as text, a deeper shade carries the text and the brand colour stays as the edge or stripe. White on `#3685F5` is 3.6:1, so the action fill is `#2563D8`. Slate is 4.4:1 on Paper Blue, so muted text is `#4F6079`.
- The test `theme::tests::critic_palette_meets_wcag_aa` holds text, muted text, links and chip text to 4.5:1, and the focus ring to 3:1, on every Critic surface.
- The page keeps the document as the hero. The page is centred on a calm blue-grey canvas, the rail sits at the right, and nothing covers the bottom of the page.

## Desktop layout

```text
┌─────────────────────────────────────────────────────────────────────┐
│ InkWitness review     Filename / Student / Assignment   Report ▾   │
├───────────────────────────────────────────────┬─────────────────────┤
│ [WordCraft rendered paper, current revision] │ [Comments / Signals] │
│                                               │                     │
│   The document grows in place.              ┌─┤ 10:42 PASTE         │
│   Highlight is displayed only when visible. │ │ 185 characters     │
│   Page scrolls smoothly when needed.        │ │ source unknown     │
│                                             └─┤                     │
│                                               │ details + before/after│
├───────────────────────────────────────────────┴─────────────────────┤
│ ◀  ▶❚❚  ▶  1×       10:42 / 60:00    ▬●━━━━◆━━◆━━━━━━━━━━━━━━━ │
│                                  flagged moments / skip-idle toggle  │
└─────────────────────────────────────────────────────────────────────┘
```

This is a textual wireframe, **not** an implemented UI. On small screens collapse the comment rail into a synced overlay/sheet but keep the timeline operable. On reduced-motion settings, replace animated scroll with deterministic reposition after user permission/accessibility preference.

## Bottom dock

- Always visible while reviewing; independent of paginated WordCraft paper. Never overlay editable content, cut off the page bottom or cover the currently highlighted text.
- Play/pause, previous/next event, previous/next flag, scrub, elapsed time, adjustable speed (e.g. 0.5×/1×/2×), idle-gap compression, content changes toggle, keyboard shortcuts, and text labels/accessible focus states.
- Two timeline layers: quiet edit density and colored/shape-coded **exact flagged event pins**. A flag pin's `eventId`, `gestureId`, `seq` and before/after anchor map directly to data. Positions are based on recorded elapsed time; a speed-compressed timeline may display a separate mapping and must not corrupt event IDs.
- Clicking one paste pin seeks to **the exact committed paste operation**, not the closest event in time; pause and show 0–1 state before event and the after state with inserted span highlighted. Show before/after excerpts and why the flag fired.
- A rapid typing burst must animate at human-viewable speed without reflowing the same entire document on every millisecond; coalesce visually when appropriate **without changing the stored event sequence**.

## Smooth scrolling and visibility

The scroll model is **viewport-aware**:

1. Reconstruct model state at the requested event, first via latest validated checkpoint then sequential replay.
2. Resolve operation anchor to WordCraft's current layout page and rectangle. If anchor no longer resolves (deleted/reflowed) show a fallback with uncertainty, never pick an unrelated text match.
3. Check rectangle against the document's visible viewport, leaving a top/bottom comfort margin (e.g. 15–20%). **Do not scroll if already visible.**
4. If off-screen, pan vertically to the nearest comfortably visible target over a moderate interval (start around 350–700 ms for a page or less; cap for longer distances); use easing and interrupt on user interaction. Avoid jumping from page 4 to page 1 and back for adjacent edits.
5. Reveal the specific insert/delete/format delta only **when its anchor becomes visible**; attach the relevant observation card in the side rail at the corresponding line/page. If an event occurs across pages, emphasize its visible part and make the rest accessible from the inspector.
6. If user manually scrolls while playing, pause or suspend auto-follow until the user resumes (configurable). No scroll oscillation for rapidly alternating anchors.
7. `prefers-reduced-motion` must disable smooth animated scrolling, but retain accurate positioning and event highlighting. Keyboard, screen-reader and high-contrast access are required.

**Crucial:** Playback state and layout geometry are separate. Page numbers, coordinates and scrolling are derived from the current reconstructed document, not stored as authoritative event anchors in the DOCX. This avoids misalignment when a teacher changes window width or zoom.

## Right-hand comment rail

- Distinguish **Word's genuine comments** from **InkWitness observation cards**. Observation cards are not themselves student misconduct findings.
- Default categories: `clipboard paste`, `large actual insertion`, `large actual deletion/replacement`, `imported pre-existing document`, `recovery gap`, `possible source overlap`, `citation/reference lead`, `policy acknowledgment`, and `consistency mismatch`.
- Each observation shows source (actual editor / inferred analysis / user declaration), exact time/sequence, scope in words and characters, before/after context, verification status, and a clear caveat.
- For AI-style indicators show named matched phrase or textual pattern **only as an interview lead**; never a fabricated probability.
- Make notes possible in reviewer memory, but do not silently mutate or overwrite the student's original DOCX. Export teacher commentary to a distinctly named review copy or printable report on explicit request.

## Review report

A compact header provides:
- Student-entered identity, assessment, local writing date and attempt ID; policy-version declaration status.
- Number of recorded operations, true paste gestures, actual large insertions/deletions, unobserved imports, recoveries/failed saves, possible source overlaps, external edits, and time covered/unknown.
- `Local history consistent` vs `External changes` vs `History missing/corrupt`, and a separate `Independent receipt checked?` field. No overall AI or misconduct score.
- Flags are clickable and jump to exact timeline event and text anchor.
- Neutral questions for an interview are generated from *specific evidence*: "What source did you use for this inserted passage?", not "Why did you use AI?".

## Mandatory playback tests

- 185-character pasted paragraph, one gesture: one exact pin tied to the correct text state and sidebar card.
- Replace paragraph with rich formatting, add hard break, heading style change: no fake mass deletion due to DOCX/JSON serialization.
- Scroll offscreen event from page 3 to page 7 smoothly without flashing offscreen changes; if already on page 7, no scrolling.
- Reverse scrub, undo/redo, deletion then reinstatement, formatting changes and anchor removal.
- User manual scroll during playback; reduced-motion; zoom; narrow laptop viewport; keyboard-only.
- 2,000–3,000 word essays with thousands of operations; memory use, responsiveness, no false page oscillation.
