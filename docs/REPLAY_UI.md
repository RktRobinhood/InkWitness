# Teacher replay UX — required behaviors

**Design principle:** A teacher watches *the original WordCraft-rendered document develop*, not a video of a separate text pane. Replay is read-only. The document remains the focus, diagnostic comments occupy a right-hand rail, and playback controls form a **fixed bottom dock like a video player**.

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
