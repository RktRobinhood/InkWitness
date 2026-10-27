# Resilient local storage and recovery (release-blocking)

## Guarantee level

Goal: avoid losing student drafts because they forgot to click Save, refreshed, closed the tab, or experienced a recoverable browser crash. **No serverless web app can guarantee survival of device loss, user-cleared site data, browser-profile deletion, private-mode disposal or every sudden power failure.** Explain this distinction in the UI.

Never show a misleading `All saved` when only in-memory state exists. Distinguish `Saved locally through revision #N`, `Backed up to chosen file`, `DOCX exported` and `Submitted to school` (the last is outside this static app and not verified).

## Local-first write-ahead journal

1. At every **committed content transaction**, issue a unique ID/sequence before announcing the action as persisted. Maintain an in-memory queue of events and associated document state.
2. Write short batches to IndexedDB within a target **<=500ms** of each content change under normal conditions, with an explicit bounded maximum queue and dirty indicator; record `persistedThroughSeq` only once the IndexedDB transaction commits successfully. Performance targets are provisional and subject to WASM/browser tests.
3. Ensure the **event batch + corresponding head pointer + updated draft metadata** are committed atomically in one IndexedDB transaction. Checkpoint snapshots are written with checksum/version and are activated only after validation. Retain **at least three viable snapshots**; never delete the last good snapshot before the replacement commits.
4. Checkpoint on a bounded interval or number of events (initial guideline: 30–60 seconds or ~100 edits), plus before deliberate exports and major imports. Do not create bogus writing events for idle checkpoints.
5. On recovery, load the newest valid head, verify chain and checkpoint, replay committed events after the checkpoint, and show a concise summary (including unpersisted changes, if known). If a checkpoint corrupts, fall back to earlier ones without overwriting intact evidence.
6. Use `visibilitychange` and `pagehide` to request opportunistic flush. **Never rely exclusively on unload handlers**, which are unreliable on browser crashes and mobile suspends.

## Browser permissions and diagnostics

- Request `navigator.storage.persist()` as an **optional user-approved action** after explaining why; do not claim the browser must grant it. Inspect `navigator.storage.estimate()` and handle `QuotaExceededError`, storage-disabled errors and transaction aborts.
- The browser's best-effort IndexedDB data may be evicted. Safari and private browsing have additional constraints; do not assume cross-device sync.
- Detect and warn on storage failures; offer `Download recovery copy` while in-memory state still exists. Never erase editor contents just because IndexedDB failed.
- On stable-origin update, migrate schemas transactionally; preserve the previous version until verified. Avoid changing `https://.../repo/` paths/origins without migration instructions.
- Enforce a **single active writer per attempt** using Web Locks API where available and BroadcastChannel coordination for conflict warnings, with safe fallback. A second tab must not overwrite the first tab's journal. Distinct students on the same browser must receive distinct attempts and drafts.

## Local project-file backup (optional, recommended)

The normal portable hand-in is a DOCX. Separately, a student may choose an **editable local recovery project** stored in a user-approved file. If Chromium's File System Access API is available and the student grants permission, the app may update this file periodically with throttling. Otherwise supply a one-click downloadable recovery copy. File-picker permissions may expire, and downloaded files do not update themselves. Offer **Open recovery file** as a visible path after browser resets.

Do not overwrite the student-selected existing file without consent. Store no file handles in the exported DOCX. Reopen project files only after validating schema, assignment and attempt identity and preventing cross-student history inheritance.

## Student-facing statuses (examples)

- `Saved on this device · through 10:42` — IndexedDB acknowledged through current sequence.
- `Saving recent changes…` — bounded queue pending.
- `Local storage failed · Download a recovery copy` — persistent urgent warning; keep editor live.
- `Backup file updated at 10:45` — independent selected file successfully written.
- `Word document downloaded` — only after triggering download; do not claim it reached Teams/LMS.

## Recovery verification tests

- Force refresh while typing, immediately reopen, verify persisted-through revision and reconstruction; measure actual worst-case lost characters.
- Kill renderer process (not graceful close), reopen same origin and restore valid last checkpoint plus journal.
- Suspend tab mid-IndexedDB transaction, reject quota, revoke persistent storage, simulate corrupted newest snapshot.
- Two tabs simultaneously; student B signs in after student A on same laptop; duplicate filenames, change identity, same assignment code.
- Website update, IndexedDB schema migration, browser cache clear, private-mode expiration, imports from other machines (honest fallback UI).
- Simulate export failure and browser picker cancel; the in-browser journal must remain intact.

**Reference:** https://developer.mozilla.org/en-US/docs/Web/API/Storage_API/Storage_quotas_and_eviction_criteria and https://developer.mozilla.org/en-US/docs/Web/API/StorageManager/persist.
