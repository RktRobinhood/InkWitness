# V2 roadmap — Microsoft 365-backed teacher-to-student-to-teacher workflow

**FUTURE ONLY — do not build during V1.** Written from the 27 October 2026 design discussion. This is the user's intended direction, not a claim that any particular Microsoft tenant already has the necessary permissions.

## Vision

After the stand-alone two-page InkWitness proof of concept works and has been deployed, turn InkWitness into a full **school Microsoft 365-based assessment workflow** while retaining its WordCraft writing surface, recording features, standards-compatible DOCX export, teacher replay, and local-first reliability.

The teacher begins by **designing and publishing** an assignment. A student signs in with the school's Microsoft account, **sees assigned work**, opens it, completes the identity/academic-honesty initialization, and writes in InkWitness. Drafts are always locally recoverable and, when permitted, safely backed up into the school's Microsoft 365 tenancy. On hand-in, a separate finalized evidence-bearing DOCX goes into a teacher-controlled Teams/SharePoint workflow. The student keeps a copy. The teacher signs in, sees received submissions, opens the same replayable evidence, comments and assesses. The teacher need not manually import a stream of unrelated downloads.

## End-to-end journey

```text
Teacher Microsoft 365 sign-in
  -> Teacher designer (essay / later multi-field questions, rubric, policy, timer)
  -> Publish to class or assigned students (Teams assignment or approved equivalent)
  -> Student Microsoft 365 sign-in
  -> My assignments / choose assignment
  -> Session initialization (policy, disclosures, confirmation, identity from login)
  -> InkWitness Write (WordCraft + granular replay events)
       |-> browser IndexedDB journal (instant recovery, offline queue)
       |-> approved OneDrive/SharePoint working draft sync (async, never blocks typing)
  -> Student finishes and confirms academic integrity
  -> Freeze/export distinct final DOCX with embedded history and receipt metadata
       |-> student retains their own copy
       `-> submit copy to Teams/SharePoint in school-controlled location
  -> Teacher Microsoft 365 sign-in
  -> Assigned submissions/dashboard
  -> InkWitness Review (WordCraft rendering + video-style replay + neutral flags)
  -> Teacher feedback, rubric and disposition; preserve original submission
```

## Workstreams and requirements (not V1 tasks)

### V2-A: authentication and authorization

- School Microsoft Entra ID sign-in, using the supported MSAL browser flow (authorization code with PKCE) and tenant-approved app registration; no client secret in public static JavaScript.
- Actual account identity supplies user IDs; do not use free-text student name for authentication. Friendly name can still appear in the document header, with careful personal data handling.
- Different student, teacher, school-admin permissions. Tenant ID, class membership, educator assignment authorization, and access restrictions must be independently checked; a user-controlled UI role tag is NOT authorization.
- Sign out, token expiry and offline behavior must not corrupt a draft. Local recovery must work when no Microsoft token is available.

### V2-B: assignment designer and publisher

- Teacher designs a TOK essay or future multi-question assessment: prompts, sources, formatting, word targets, optional timer, rules, AI allowance settings, rubric, permitted reference material, student-visible instructions.
- Preview the exact assignment as a student before publishing; version every published assignment.
- Target school class, course, assignment and due dates. Determine whether Microsoft Teams for Education `educationAssignment` APIs are usable with school IT consent; an approved SharePoint-based assignment register is an alternate option if permissions or capability block the Teams path.
- Explicit states: `draft` / `published` / `closed` / `archived`; students must never have permission to edit the teacher's assignment template or another student's response.

### V2-C: student assignment dashboard and initialization

- After sign-in, students see only work assigned to them, with due date/status and launch/resume choice.
- A student's attempt has an immutable assigned identity and versioned assignment ID. Carry forward V1's hard session separation, isolation on shared machines, onboarding, academic-honesty declaration and disclosure of locally recorded process data.
- Start/resume/import choices must not silently claim externally prepared content was typed during the monitored session.

### V2-D: local-first write and Microsoft 365 synchronization

- The browser IndexedDB journal remains PRIMARY for immediate recovery; cloud upload is an **asynchronous second backup**. Preserve both document and revision log, with storage-status UI distinguishing 'saved on this device', 'uploaded to school cloud', 'awaiting upload', and 'conflict'.
- Use least-privilege Microsoft Graph delegated operations for a school-approved student OneDrive or SharePoint library. File location, retention, version history, sharing and permissions are institution settings, not assumptions.
- Use version/ETag preconditions or equivalent conflict detection. Never overwrite a newer remote draft silently, merge two students' histories, or drop locally queued edits on token expiry. New device resume must clearly show provenance and reconciliation.
- WordCraft DOCX remains the standard portable output. V1's replay format is not replaced by proprietary account-only history.

### V2-E: hand-in and teacher delivery

- Create a FINAL, distinct evidence-bearing DOCX at submission; preserve the original. The student keeps a copy.
- Send the school copy through the approved assignment mechanism (prefer Teams Education if authorized and suitable, otherwise school-managed SharePoint submission folders/metadata). Verify hand-in completion using a response from Microsoft APIs; do not report success merely because a browser initiated upload.
- Teacher sees submission time, assignment version, identity and actual received file; student cannot change the teacher-controlled submitted copy after hand-in without a versioned resubmission event.
- Teacher dashboard shows assigned classes, received/missing/reviewed submissions, filtered flags, reviewer launch, feedback and assessment status. Use role-checked access to files and reports.

### V2-F: integrity evidence and review

- Reuse **the same** event schema, replay UI, DOCX Custom XML parts and neutrally worded diagnostics from V1. Avoid any 'AI probability' verdict.
- Optional school-controlled authenticated event receipts or timestamp witnesses may improve independent provenance, if approved. Microsoft file/version timestamps alone are NOT proof of who authored every keystroke.
- Consider immutable or retention-governed school records with least-privilege access, audit logs and explicit retention expiry. Do not claim cryptographic security unless the underlying server-side trust source is verified.

### V2-G: multi-stage assignments

- Extend from one continuous TOK essay to teacher-designed sections, multiple response fields, math-working surfaces, attachments and staged prompts without losing the single-document editing feel.
- Evolve the schema in versioned fashion: `assignmentId`, `assignmentVersion`, `attemptId`, `fieldId`/`sectionId`, stable operation IDs, and per-field replay indexing.
- Rendering a multi-part assignment as one DOCX should remain possible, with distinct section-level teacher comments and seek markers.

## Proposed modular interfaces, V1-compatible

Do NOT prematurely code these interfaces in V1 solely for V2. The V1 components should keep clean boundaries so that an adapter can be introduced later.

```text
IdentityAdapter       : anonymous local (V1) -> MS Entra school account (V2)
AssignmentSource      : local TOK template (V1) -> school assignment list (V2)
DraftStore            : IndexedDB/local backup (V1) -> local + Microsoft cloud sync (V2)
SubmissionTransport   : manual DOCX hand-in (V1) -> Teams/SharePoint (V2)
EvidenceVerifier      : local consistency/optional separate receipt (V1)
                     -> school-received receipt and policy-controlled verification (V2)
ReviewSource          : teacher local-file picker (V1) -> authorized submitted documents (V2)
```

All implementations still use the **one portable DOCX** format and share the identical replay engine.

## Implementation sequence after V1 approval

1. **V2-0 tenant feasibility spike:** school IT approval; confirm app registration, delegated scopes, teacher/student test accounts, Teams education licensing and actual APIs accessible; compare Teams/SharePoint delivery; document GDPR conditions and regional storage.
2. **V2-1 Microsoft sign-in:** valid roles, sign-out, token refresh, no secrets in public frontend, no cross-student leakage.
3. **V2-2 assignment catalog/designer/publishing:** classes and permissions, versioned assignments, student dashboard.
4. **V2-3 resilient OneDrive/SharePoint draft sync:** IndexedDB offline/recovery plus concurrency and conflict resolution.
5. **V2-4 automatic final submission + teacher dashboard:** independently confirm the server received the DOCX and teacher can access it.
6. **V2-5 review integration, reporting, school pilot:** role permissions, records retention, feedback, audit and accessibility.
7. **V2-6 multi-field assignments:** only after the end-to-end essay workflow is stable.

## V2 non-goals / warnings

- V1 must not wait for this roadmap. A separate Microsoft 365 sign-in is **not needed** to use V1.
- A hidden static `/review/` URL is not protection for online student records; V2 requires genuine account-based authorization and permissions on the underlying files.
- Microsoft Graph offers components, NOT a complete turnkey assessment application. School tenant policies and Graph permissions can block the desired flow; use a tested fallback rather than imagining availability.
- Public GitHub Pages can host a frontend that uses Microsoft sign-in, subject to tenant approval, but public client-side code must never contain secret keys or application-level permissions. A custom backend may be required for server-only Graph operations or trusted signing. Explore that ONLY in V2.
- No AI agent has permission to complete student essays merely because the development repository has open coding-agent instructions. Student-facing AI guidance is separate and non-binding to non-cooperating external agents.

## V2 acceptance example

A teacher signs into the approved school tenant, creates an essay assessment, publishes it to a specific class, and confirms a particular student can see it. That student signs in, starts a fresh attempt, writes on school day with local recovery, loses their connection temporarily, resumes without losing text, syncs successfully, finishes, obtains their DOCX copy and submits a separate one to the teacher. The teacher sees the received document in a role-protected dashboard and replays its original writing-day process at the correct paste marker. Word itself still opens that submitted DOCX as a normal essay. No part of V2 makes an unsupported authorship verdict.
