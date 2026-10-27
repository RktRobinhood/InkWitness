# Test matrix and evidence requirements

**Purpose:** Prevent a technically valid but pedagogically misleading release. Results must be `PASS`, `FAIL`, or `NOT TESTED`, with real browser version, platform and fixture; no synthetic claims of test success.

## A. Core student writing and provenance
| ID | Scenario | Required result |
|---|---|---|
| W01 | Type `Knowledge matters` one character at a time | Distinct natural insert operations, no large-insert or paste false flag. |
| W02 | Paste exactly 185 characters using Ctrl+V | One paste gesture linked to exact committed operations; pin seeks to exact insertion; before/after text matches. |
| W03 | Paste rich HTML with multiple paragraphs and formatting | Preserve content and formatting; no phantom 500-character deletion based on paragraph JSON rewriting; paste provenance remains `unknown`. |
| W04 | Delete two words in the middle, then Undo and Redo | Deleted span highlights correctly and reversibility holds; word count and sequence correct. |
| W05 | Change a heading to a paragraph, insert a hard break, split a paragraph | Structural edits are not reported as huge deletions/pastes unless actual text changed. |
| W06 | Select two paragraphs and replace with one line | Real large replacement recorded with original and new text; flags reference exact event. |
| W07 | Copy within the document then paste elsewhere | Real copy/paste semantics retained; internal copy hint explicitly unverified/claimed if used. |
| W08 | Paste from Word, Google Docs, webpage, plain text, ChatGPT output | No unsupported claim identifying the source app; record only MIME types/available hints. |
| W09 | Input Japanese/Korean/Chinese through IME and English via mobile virtual keyboard | Composition not misdiagnosed as instant AI input; valid resulting text/replay. |
| W10 | Rename student, resume browser, open second tab, import an existing document | Distinct attempt IDs; no silent takeover; explicit unobserved import boundary and conflict/recovery warning. |
| W11 | Start two students sequentially in same browser, same site URL | Zero inherited history, session ID or previous student's identity in second attempt. |
| W12 | Disable paste in a teacher-configured practice assignment | Blocked attempt reports only intended metadata; no clipboard-content surveillance. |
| W13 | Storage quota exceeded / IndexedDB unavailable | Visible failure and backup download; no false `Saved` status. |
| W14 | Close/reopen after 1,000 edits and browser refresh | Document + event sequence restored; monotonic elapsed timing restarts in a new span. |

## B. Viewer and replay navigation
| ID | Scenario | Required result |
|---|---|---|
| R01 | Import WordCraft-generated DOCX with 185-char paste | Open in real WordCraft reviewer; exact event/card/text alignment after seek. |
| R02 | Play 5 pages of simulated writing from start to finish | Bottom transport remains fixed; real pages change, comment cards update with events. |
| R03 | Next edit lies below current viewport | Smooth internal scrolling occurs first; then visible-only change/highlight and card reveal; no outer-page jump. |
| R04 | Next edit lies above viewport | Same smooth bounded scrolling, no repeated oscillation. |
| R05 | Rapid edits within one visible paragraph | No repeated re-centering; caret and highlight stay readable. |
| R06 | Click dense/overlapping timeline flag pins | Expanded exact selectable events, deterministic seeking by seq rather than approximate timestamp. |
| R07 | Replay after document reflow due to inserted paragraph or table | Logical anchor follows content; no stale page number target. |
| R08 | Pause, single step back, 2x, 4x, idle skip | Controls operate and preserve deterministic final state; labels are clear. |
| R09 | `prefers-reduced-motion`, keyboard-only, screen reader | Motion suppresses; focus order and event narration remain usable. |
| R10 | Teacher creates a note and closes page | Source DOCX unchanged; optional explicit exported review file can preserve teacher note. |

## C. DOCX compatibility and evidence
| ID | Scenario | Required result |
|---|---|---|
| D01 | DOCX generated from blank essay | Opens in Microsoft Word desktop as ordinary editable DOCX, with normal paragraph formatting, header and watermark. |
| D02 | DOCX contains tracked insertions/deletions | Microsoft Word Review pane reads native revisions; full InkWitness replay reads from Custom XML. |
| D03 | DOCX open/edit/save in Microsoft Word for Windows | Custom XML retained or loss reported; original replay reconstructs; external edit displayed without fake timeline. |
| D04 | Microsoft Word for Mac edit/save | Same, platform recorded. |
| D05 | Word for the web / OneDrive edit/save | Same, explicitly test rather than assume preservation. |
| D06 | LibreOffice edit/save DOCX | Same, record component/version; no generalization from one test. |
| D07 | WordCraft DOCX resave | Preserve custom XML relationships and reconstruct history. |
| D08 | Save to PDF, RTF or ODT | Explain replay metadata not portable in those formats, normal document still usable. |
| D09 | DOCX strips custom XML part | Viewer shows document and honest `Replay unavailable`. |
| D10 | DOCX XML or event-chain corrupted / ZIP bomb / external relationship | Reject safely, do not execute active content or fetch remote URLs; show precise import error. |
| D11 | DOCX edited after tracked session; chain intact | Viewer displays historical replay and separate static external diff; does not invent recording. |
| D12 | Header or name metadata differs from evidence identity | Display consistency warning; do not infer student deception. |

## D. Academic-integrity and privacy review
| ID | Scenario | Required result |
|---|---|---|
| P01 | Paste 180 chars containing quote and citation | Neutral clipboard observation, not misconduct label; can review source/citation context. |
| P02 | Phrase pack matches many common L2 English expressions | Counts/excerpts only; no AI probability, no automatic disciplinary classification. |
| P03 | Student pastes document from a permitted external editor | Explicit provenance limits, no supposed ChatGPT origin. |
| P04 | Reviewer is open on public static route | No automatic upload, remote logging or server student-file persistence; unlisted URL not described as authentication. |
| P05 | Student receives policy information | Student-facing notice clearly explains edit logging and AI rules; machine-readable policy does not impersonate system messages. |
| P06 | Tamper by rewriting DOCX and recalculating hashes | Report unsigned local consistency limitation; do not show `authenticated`/`author verified`. |

## E. Exit criteria and evidence artifacts
- G0 requires screenshots or a screen recording from the **real WordCraft WASM editor**, a deterministic replay fixture, OPC inspector output and a Word open/edit/save result with the actual version.
- G1 requires a reproducible `sample-essay.docx` and per-event inspection output showing the exact 185-character paste mapping.
- G2 requires automated seek tests and a visual demonstration of smooth scrolling to an offscreen event with synchronized comment card.
- G3 requires a full matrix (`PASS`, `FAIL`, `NOT TESTED`) on supported browsers/devices, performance measurements, security checks and a neutral academic-integrity language review.
- At all milestones, `NOT TESTED` is preferable to claiming unperformed compatibility tests; do not accept `replay works` if only an unrelated HTML text snapshot was played back.

## F. Mandatory identity, policy, export, and robust recovery (new release gates)
| ID | Scenario | Required result |
|---|---|---|
| N01 | New student document, no browser draft | Policy/telemetry disclosure displayed first; student full name and assessment title mandatory. |
| N02 | Two required identity fields empty | Writer cannot begin a new assessed attempt until filled; no invented anonymous identity. |
| N03 | Resume an existing attempt | Clear resume/new/import choices, student metadata displayed; no silent transfer to another student. |
| N04 | First intentional `File > Save` or `Finish & submit` | Initially unchecked academic-honesty checkbox; export requires positive action, records exact declaration/version/time in embedded XML. |
| N05 | Keep writing for 30 minutes **without** checking manual-save declaration | Automatic browser recovery continuously persists. User can recover even before the first deliberate DOCX save. |
| N06 | Successful initial export | DOCX normal `core.xml` title/creator and custom props contain assessment/identity where applicable; Word header matches; faint watermark is visible but not intrusive. |
| N07 | Modify header/properties in Word after export | Viewer reports metadata consistency warning and any unobserved content change; does not assert malicious intent. |
| N08 | Browser crashes with queued edits | Restore last committed journal + checkpoint; no false `All saved`; UI states last persisted revision. |
| N09 | Corrupt latest checkpoint but leave earlier verified checkpoints | Recovery falls back to last valid checkpoint and sequential event log; original work not discarded. |
| N10 | Simultaneous tabs editing same `attemptId` | Second writer is prevented from overwriting the first or prompted to fork a new attempt. |
| N11 | `navigator.storage.persist()` denied; quota almost full | Honest status and portable backup controls; no unsupported `permanent save` promise. |
| N12 | First student exits; second student uses same browser at school | New attempt and data isolation; no inherited edits, paste flags or policy declarations. |
| N13 | Open `/review/` offline after page assets are cached | Works only if actual offline mode is implemented and tested; otherwise accurately requires static site load. |

## G. Local analysis / AI-agent policy / separate trust boundaries
| ID | Scenario | Required result |
|---|---|---|
| A01 | Teacher supplies source excerpt and student pastes 150+ characters | Winnowing/n-gram overlap provides matching source location, threshold and excerpt; correctly cites a quotation as a match but not plagiarism. |
| A02 | Teacher supplies no source corpus | Source overlap marked `not performed — no corpus`, not `no plagiarism found`. |
| A03 | Paste rich HTML with apparent ChatGPT/Google source markers | `sourceHint` explicitly unverified, no reliable source-application attribution. |
| A04 | Check wording-pattern module | Named rule, exact matched text and limitations only; no AI percentage, generated plagiarism verdict or automatic sanctions. |
| A05 | New page is loaded by cooperating AI tool | Public human/agent guidance available as data and visible notice, but no assertion the agent was technically blocked. |
| A06 | Open browser dev tools or extension-controlled edits | No claim that the static page can prove the identity of the editor/agent; test surface for honest limitations. |
| A07 | No network permissions; run local analysis | No document text, clipboard or analysis output transferred to host/third parties. |
| A08 | Remotely hosted optional source checks disabled | No requests to Crossref/OpenAlex or model endpoints unless teacher has expressly enabled a later feature. |
| A09 | Recompute every in-DOCX hash after editing both essay and log | Local checks may be consistent; reviewer still says `independent receipt not checked`, never authenticated. |
| A10 | Provide an independently saved original export SHA-256 from the school system | Exact byte comparison reports match/mismatch; modified by ordinary Word save may mismatch despite honest use and must be explained. |


## BRAND: release-blocking InkWitness identity checks

- `assets/brand/inkwitness-app-dark.svg`, `inkwitness-app-light.svg`, `inkwitness-write.svg`, `inkwitness-review.svg`, `inkwitness-lockup.svg`, `favicon.svg` and PNG/ICO variants exist and render cleanly.
- 16/32px favicon legible; 192/512px icons remain crisp, correct aspect ratio, no watermark or hidden badge outside viewport.
- `/write/` document title, visible brand, favicon and loading states say **InkWitness Write**; `/review/` say **InkWitness Review**. Check both with project-page base paths.
- No ArtCraft/WordCraft trademark **graphics** remain in the distributed branded app; plain-text “Built on WordCraft” attribution and original MIT/Apache legal notices remain. Do not replace ordinary editing-command icons.
- Test both light and dark, high DPI, keyboard access and 200% zoom. The brand preview page is illustrative only and not an editor build.
