# V2 Microsoft 365 feasibility and security checklist (future only)

**No account, tenant, connector or consent has been verified for this starter.** This file tells a future implementation agent what to determine with school IT before writing cloud integrations. V1 must remain functional without any of it.

## Confirm with the school

- Confirm school owns/approves a Microsoft 365 Education tenant and the teacher/student account types, eligible licenses and applicable permissions.
- Confirm an administrator can register an application in Microsoft Entra ID (or grant approved consent), and the redirect URIs for GitHub Pages / approved host.
- Confirm the school's policy for app data processing, retention, student age, school data classification, Danish/EU GDPR basis, data residency and incident response.
- Determine which class/roster/assignment operations the configured Teams Education deployment actually exposes to the application's delegated scope. Do not assume `educationAssignment` create/list/submit endpoints work for every tenant or role.
- Establish whether SharePoint site, library, Lists and teacher/staff permissions can be managed by approved school administrators. Do not accidentally make assignments or documents world-readable.
- Decide whether teacher hand-in should use Teams for Education assignments, SharePoint library or the school's existing submission channel. Test the complete flow with real teacher/student accounts in a sandbox class.

## Auth and permissions gate

- Use supported MSAL Browser + authorization code/PKCE where possible. Avoid implicit flow, local static secrets and embedding user credentials in a document.
- Use minimal delegated Microsoft Graph permissions, authenticated roles and Microsoft-backed assignment/file ACLs. Discover exact permission names and review least privilege against CURRENT official docs during implementation; do not hardcode guessed scopes in V1.
- Prevent account/attempt confusion on shared computers: identity changes require isolation and an explicit continue/recover workflow.
- Tokens and student data must not appear in logs, public URLs, crash reports or a published repository.
- Handle revoked consent, suspended accounts, dropped network, token expiry, OneDrive capacity, blocked third-party cookies and changed school policies without losing an existing browser draft.

## Data model / evidence gate

- Identity: immutable tenant-specific account ID, class/assignment/student attempt mapping; display name and header are not authorization signals.
- Authoring: immutable publication version, teacher-owned source prompt and editable student response kept in separate principals/locations.
- Storage: local events + cloud draft; preserve original final submission; resilient conditional updates and duplicates/idempotency.
- Submission: separately received teacher copy with confirmed Microsoft service response, status and access checks. Do not use a student-managed editable link as if it were immutable hand-in.
- Integrity: event sequence hashes can show local consistency; independent receipt requires school-controlled storage/time/signing. A response from the cloud service confirms receipt of bytes, not true keyboard authorship.
- School retention/deletion policy for identifiable essays and granular edit timelines; separate research and classroom use approvals where required.

## Official references to re-verify during V2 kickoff

- Microsoft identity platform / MSAL for single-page apps: https://learn.microsoft.com/en-us/entra/identity-platform/scenario-spa-overview
- Microsoft Graph education overview: https://learn.microsoft.com/en-us/graph/education-concept-overview
- Microsoft Graph education assignment resource: https://learn.microsoft.com/en-us/graph/api/resources/educationassignment
- Microsoft Graph OneDrive / DriveItem concepts: https://learn.microsoft.com/en-us/graph/api/resources/driveitem
- Microsoft Graph permissions reference: https://learn.microsoft.com/en-us/graph/permissions-reference
- SharePoint/OneDrive file versioning: https://learn.microsoft.com/en-us/sharepoint/document-library-version-history

These are architectural pointers, **not** confirmation of current tenant eligibility or API availability.
