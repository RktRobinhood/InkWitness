# Browser-local academic-integrity observations — selected free/open-source stack

**Product rule:** The goal is a faithful, useful *writing-process report*, not an AI-cheating accusation generator. A diagnostic must have a named rule, evidence, exact event reference or document location, explainable finding and limitation. The reviewer never displays an AI authorship percentage.

## Components selected for the MVP and follow-on

| Role | Method/library | License | Timing | What it can actually say |
|---|---|---|---|---|
| True paste/delete/cut/undo and structural edit events | **WordCraft semantic operations + browser ClipboardEvent where available** | WordCraft MIT OR Apache-2.0 | **P0 / must ship** | A clipboard paste happened in this editor; what document content the operation inserted or removed; source application unknown |
| Exact/near-exact reuse from provided sources | **Winnowing / character or word n-gram fingerprints** implemented in-house; optional **MinHash** when corpus grows | Algorithm open; own implementation MIT | **P1** | This passage overlaps a specific locally supplied reference, with the matching passage and source displayed |
| Explanatory text features | **winkNLP** with compatible bundled language model; verify model license and size | Library MIT; separately check model license | **P1** | Repetition, sentence boundaries, vocabulary/syntactic differences, writing-structure cues, not AI authorship |
| Human-readable inserted/deleted span | **WordCraft operations**; optional **jsdiff** for presentation only | WordCraft dual-license; jsdiff BSD family (verify package distribution) | **P0** | Display actual text before/after, respecting Unicode and semantic paragraph structures |
| References and DOIs | Local DOI/URL/citation syntax checks; optional school-approved external metadata search | Hand-written; Crossref/OpenAlex API terms separately | **P1 local, P2 remote** | Bibliographic record potentially exists or needs checking; never claim a source supports the quoted page without obtaining it |
| Optional small semantic model | **Transformers.js** + specifically licensed local model bundled into build | Library Apache-2.0; model license varies | **P3 / deferred** | Optional semantic similarity with known sources, not calibrated authorship probability |

Do **not** install an opaque AI-writing detector, generic MIT-branded phrase list or downloaded model of unclear license. No validated “MIT AI detector” has been identified as a ready-to-ship open-source browser package. Do not label ordinary wording observations as an MIT statistical assessment.

## Trigger strategy

- **On substantial paste**: observe actual clipboard event -> `gestureId` -> committed document operations; compute characters/words inserted; if threshold met, run local analyses **asynchronously** in a browser worker after the edit persists. Avoid blocking typing or stealing focus.
- **On document import**: label the entire pre-existing text `unobserved import`. Run a one-time content/structure scan to establish source similarities if local reference materials are available. Never fabricate keypress history for imported text.
- **On teacher review**: repeat/recompute optional analyzers against stored evidence and a teacher-selected reference corpus, with analyzer version and corpus identifiers. Results may differ from the original if the corpus or model changed; label the origin of the finding.
- **On ordinary typing**: save semantic editor operations, do not run heavy style/AI models after every character.

## Three distinct evidence categories

1. **Directly observed editor activity** (`observed_event`): actual operations recorded by the student-side application; e.g. paste 185 chars, delete 14 chars, formatting change, undo. Still student-controlled browser, so “observed by this session” rather than “proves this human typed.”
2. **Source comparison** (`source_overlap`): passage similarity against known source documents supplied locally. Show quote, source, matching span, method, threshold, missing corpus limits. Matching a properly quoted/cited source is not automatically improper.
3. **Advisory language indicators** (`language_observation`): short factual patterns, conspicuous answer-template artifacts or discontinuities with sufficient context. Mark uncalibrated, low evidential value; do not compute “AI probability,” confidence of misconduct, red scores or sanctions.

## Clipboard metadata: exact limits

`ClipboardEvent.clipboardData` may expose `text/plain`, `text/html`, images or URI formats. HTML may include hyperlinks or app-specific markup, but **there is no dependable browser-standard field naming the source app or tab** (ChatGPT, Claude, Gemini, Word, etc.). If the clipboard supplies explicit URLs or generator markers, record them only as **unverified hints**, not provenance facts. Avoid storing entire clipboard payload separately from the passage actually inserted into the essay. Do not inspect unrelated clipboard history or browser extensions.

Record `clipboardTypes`, `pasteAt`, `gestureId`, inserted span (already part of document history), `sourceStatus=unknown`, optional `sourceHint`, and exact reasons. Avoid claiming agent detection based on typing rate alone; speech input, accessibility systems, IME composition, macros and normal paste can look similar.

## Suggested rule definitions (configurable teacher parameters)

| Rule | Default proposal | Output |
|---|---|---|
| `wa.clipboard.real-paste.v1` | Any true paste event | Informational exact event |
| `wa.clipboard.large-paste.v1` | >= 150 characters **or** >= 30 words in a single true paste | Highlighted event; prompt to inspect source |
| `wa.edit.large-insert.v1` | >= 150 characters added in one committed action, without recorded clipboard gesture | Neutral 'large insertion; input mechanism undetermined' |
| `wa.edit.large-delete.v1` | >= 150 visible text characters removed in one committed action | Neutral deletion; original passage retained in replay |
| `wa.import.unobserved.v1` | Any text already present when document opened/imported | Unobserved starting text, not a paste/typing event |
| `wa.source.winnowing.v1` | Locally supplied source match above explainable overlap threshold | Exact source excerpt + matching document range |
| `wa.references.doi-format.v1` | DOI-like reference fails local syntax check | Needs reference review; no remote validity claim |
| `wa.text.template-marker.v1` | Exact recognizable machine-response wrapper (e.g. “As an AI language model”) | Low-confidence contextual lead with quotation; NOT a finding of AI authorship |

Thresholds are initial UX hypotheses, **not validated detection boundaries**. Calibrate on consensual synthetic/volunteer samples, multilingual texts and accessibility contexts before classroom use. Store rule ID/version, parameters and text span used. Never hide a scoring algorithm as a secret anti-cheat trap in a public codebase.

## Local corpus architecture

- Teacher optionally supplies source PDFs/text/HTML or a curated reference pack **within the browser**, loaded in memory only. Hash source IDs, store only fingerprints and needed excerpts in review evidence if allowed. No internet-wide plagiarism claims.
- If the assignment intentionally gives students source material, matching that material can be expected: reviewers should see *citation context* and distinguish a cited quotation from unattributed borrowing.
- Cross-student similarity requires a teacher-selected batch of submitted essays processed locally; it is **not** a student-side check and requires clear institutional permission.
- Source matching must be Unicode aware and normalize spaces/punctuation with both original excerpts kept. Avoid inaccurate false matches from short common phrases.

## Reporting and privacy

Each finding includes `ruleId`, `ruleVersion`, category, exact source event ID/field/anchor, matched span, inputs available, result, limitations and status. Report checks as `performed`, `skipped due to missing sources`, `disabled`, or `failed`; never imply a check ran if it didn't. No student prose is sent to remote services in the static MVP. A browser worker still runs **locally**. If external citation metadata verification is added later, require explicit teacher approval and disclose third-party requests.

## User review / ethics

- Do not create AI-authorship percentages or automated integrity sanctions; these can mislead and disproportionately burden multilingual students and students using assistive technologies.
- Offer teacher interview questions grounded in actual edits. The student should be able to explain innocent paste/revision practices.
- Preserve and make clear the difference between **academic misconduct**, **observable process events** and **invalid/ambiguous logs**.
- Any pattern module requires academic/teacher QA before being enabled by default. The minimum MVP can ship with objectively recorded actions only.

**Useful sources:** https://github.com/winkjs/wink-nlp, https://github.com/components/jsdiff, https://huggingface.co/docs/transformers.js/en/index, https://www.w3.org/TR/clipboard-apis/.
