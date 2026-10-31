# ADR 0003: Sessions and unobserved spans (evidence schema 1.2)

- Status: accepted (FAL-662). Base for keeping history across sessions (GH #9) and for outside changes (GH #10).
- Date: 2026-10-31
- Scope: `schemas/evidence-v1.schema.json`, `crates/inkwitness` (`evidence.rs`, `replay.rs`), `examples/evidence-v1.2-unobserved.json`.

## Context

A file can be opened more than once, can start from text that was already there, and can be changed by something other than the InkWitness writer. In each case there are stretches the recorder did not see. The reviewer must be able to say so by name, with where each starts and ends, and must never show "unknown" as clean or as suspicious.

The schema already had `sessionSpans` (one entry written per file, always `reason: "new"`). Nothing recorded a gap.

## Decision

### Schema 1.2

`schemaVersion` takes `1.0`, `1.1` or `1.2`. A file of an earlier version validates and replays as it is. The two shapes are tied to the version: a 1.2 file **must** carry `unobservedSpans` (an empty list when nothing is missing), and a 1.0 or 1.1 file **must not**. The reader treats a missing list as "none recorded", which is not a claim that nothing was missed.

### Sessions

`sessionSpans` stays the list of sessions, oldest first, one per time the student's writer opened the attempt. 1.2 adds optional `lastSeq` (the last event recorded in the session, 0 if none), and `startedAtServer` and `endedAtServer`.

### Unobserved spans

`unobservedSpans[]` has `spanId`, `kind`, `label`, `afterSeq` and `beforeSeq`, and optional times, session ids, `fieldId` and anchors.

| `kind` | Meaning |
|---|---|
| `gap-between-sessions` | The time between the end of one session and the start of the next. Written by the writer for every pair of sessions. |
| `preexisting-at-import` | Text that was already in the document when it was opened or imported. |
| `change-outside-writer` | A change made where InkWitness was not running. The schema and the writer's pass-through (`Attempt::other_unobserved`) are in place. Detecting such a change is GH #10. |

- **Where it starts and ends** is `afterSeq` (the last event seen before it, 0 if none) and `beforeSeq` (the first event seen after it, or null if none follows). When the span is tied to text, `startAnchor` and `endAnchor` are ordinary anchors. For a gap, `startedAtLocal` and `endedAtLocal` are the end of the session before and the start of the session after.
- **The name** is `label`, in neutral words, always starting "Not observed". A reviewer shows the label as written.
- Spans are not events. They are not in the digest chain, so adding or reading them never changes an `eventDigest`, and they cannot be used to hide or forge an event. (Like the rest of the file they are unsigned local data, so they are not proof of authenticity either.)

### Client time and server time

`startedAtLocal`, `endedAtLocal` and the existing `atLocalIso` are the student's device clock (**client time**) and may be wrong or changed. Server time is separate: `startedAtServer` and `endedAtServer` on sessions and spans. V1 has no server, so the writer leaves them null. A later version can fill them in without changing what the client fields mean.

### What unobserved means

Unobserved means InkWitness has no record. It does not mean the text is clean, and it does not mean anything went wrong. Nothing is inferred about activity in a span, no keystrokes are made up for it, and no wording, label or diagnostic may present it as wrongdoing. A normal break (lunch, an overnight save) produces a gap span as a matter of course.

### Writer and reader

`evidence::Attempt` gains `reason`, `earlier_sessions` and `other_unobserved`. The writer emits 1.2 only when there is a reason to (a resumed, recovered or imported session, an earlier session, or another span). Otherwise it still writes 1.1 or 1.0, so files that never had a gap are unchanged. `Replay::sessions()` and `Replay::unobserved_spans()` give a reviewer the lists, and `Replay` verifies 1.2 files the same way as before.

## Consequences

- The recorder is still one session wide: it numbers events from 1 for the session it is attached to. Carrying the earlier sessions' events into a resumed recorder, so that the chain runs across sessions, is done in [ADR 0004](0004-resuming-history.md) (GH #9).
- Nothing yet shows the spans in the review UI. That is a follow-up on top of `Replay::unobserved_spans()`.
- Envelope `schemaVersion` (`wa:history`, "1.0") is the wrapper's own version and is unchanged.

## Tests

`crates/inkwitness/src/tests_sessions.rs`: two sessions carry two sessions and the gap between them; an imported start names the text already there; files without spans still open and replay; the fixture `examples/evidence-v1.2-unobserved.json` equals what the writer produces and passes replay and the digest chain. `scripts/validate_starter.py` validates the fixture, an older version and the version rules against the JSON Schema.
