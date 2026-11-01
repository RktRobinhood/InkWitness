# ADR 0006: Flags become Findings (FAL-665)

- Status: accepted. Builds on [ADR 0005](0005-changes-outside.md) and `docs/DIAGNOSTICS.md`.
- Date: 2026-11-01
- Scope: `crates/inkwitness-review/src/findings.rs`, `rail.rs`, `review.rs`, `comments.rs`, `export.rs`.

## Context

A flag card said what happened and gave one sentence of meaning. It offered the teacher no other explanations, no next step and no place to record what they decided. A flag could also be mistaken for a verdict: strong flags were drawn in the colour of a corrupt history.

## Decision

Each flag in the review rail reads as a **Finding** (`Finding`, `findings.rs`) in five parts: the **observed** event, the supporting **record** (the card: event, time, size), **plausible alternatives**, the **verification action** ("Check next") and the teacher's **resolution**. Every finding also shows what it **does not prove** and its **method version** (`findings.v1`) next to its rule id (`wa.….v1`), so an old report keeps its meaning when wording changes. Alternatives are written per kind; a pasted block lists quoted source, the student's own earlier draft, an assistive workflow, undeclared outside text and another explanation.

### The contract

The reader draws a `Finding` and nothing else. A later check (similarity, references, any model) adds a rule and fills these fields without changing the reader:

| Field | Meaning |
|---|---|
| `rule` | Rule id and version, `wa.<area>.<name>.v<N>` |
| `method` | Version of the findings' shape and wording (`METHOD_VERSION`); raise it when an alternative, check or limit changes meaning |
| `observed` | One sentence of what happened, as recorded |
| `record` | Event, time and size |
| `alternatives` | At least two plausible other explanations, neutral, unranked |
| `next_check` | At least one action the teacher can take |
| `not_proven` | What the finding does not prove |
| `resolution` | `Open`, `Explained`, `Needs conversation` or `Dismissed` |

### Resolution

The teacher sets a resolution (four buttons) and an optional note on the finding. It is kept as a `TeacherComment` with `resolution: Some(..)`, anchored on the event's text in the final document, so it is listed with the comments ("Finding resolution · Explained"), drawn on the page as a comment, and written into the review copy as a Word comment ("Finding · Large paste · findings.v1: Explained." and the note). Open with no note removes it. It is in memory only, like every teacher comment. A resolution is not "promoted": the flag still offers Promote to comment.

### No score

A finding has no score, no total, no ranking and no risk figure; tests hold the finding's wording and the rail's text to that. Flag chips use neutral tints: none is the colour of a corrupt history. The kind is told apart by icon and words.

## Limits

- Resolution is kept per flag (event and kind), for the open file only. It isn't saved with the student's file.
- A change made outside InkWitness ([ADR 0005](0005-changes-outside.md)) lists its flags as chips; resolving them is not offered yet.
- The wording of alternatives is a first draft for teachers to review; it was not tested with teachers.
