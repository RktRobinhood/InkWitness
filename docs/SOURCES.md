# Verified engineering references — 27 October 2026

These are **research inputs**, not claims that the application has been built. The coding agent must inspect the **pinned upstream tag**, record its full commit hash, test actual school browsers, and update the references when dependencies change.

## WordCraft source and feature boundaries

- **Canonical repository and architecture:** https://github.com/storytold/wordcraft — open-source pure Rust word processor with a complete Cargo workspace, native document model, rendering engine, DOCX read/write, Track Changes, comments, headers, watermark and web WASM app; upstream clearly labels it **alpha**. License MIT OR Apache-2.0.
- **Release v0.11.0:** https://github.com/storytold/wordcraft/releases/tag/v0.11.0 — released 25 October 2026; release lists abbreviated commit `e31f2f0`; agent to verify full commit.
- **Workspace toolchain:** https://raw.githubusercontent.com/storytold/wordcraft/main/Cargo.toml — as inspected 27 October 2026, workspace Rust 1.95, edition 2024, `crates/*`, `apps/*`, `xtask`. Use pinned tag rather than assuming main stays identical.
- **Browser host bridge:** https://raw.githubusercontent.com/storytold/wordcraft/main/apps/wordcraft-web/src/host.rs — same-origin `?host=parent` bridge with `wordcraft:ready`, `open`, `save`, `dirty`, `failed`; **no existing guaranteed committed semantic event stream**.
- **Browser app shell:** https://raw.githubusercontent.com/storytold/wordcraft/main/apps/wordcraft-web/src/web.rs — WebAssembly canvas, paste handling, save picker/anchor fallback and preferences. This is not yet the durable document-replay recording system.
- **Engine architecture:** https://github.com/storytold/wordcraft/blob/main/docs/architecture.md — `Session::run`, commands and model/layout/render pipeline.
- **DOCX Custom XML retention, limitations:** https://github.com/storytold/wordcraft/blob/main/docs/file-format-parity.md — section “Custom XML store preservation” states opaque custom XML stores and relations survive two edited save/reopen cycles in synthetic tests; real Word desktop and online behavior for our payload must still be tested.
- **Upstream agent and attribution rules:** https://raw.githubusercontent.com/storytold/wordcraft/main/AGENTS.md — no unsafe/panics, never crash, original/open assets, asset attribution.
- **Web quick start:** README documents `cd apps/wordcraft-web && trunk serve` and static wasm bundle distribution; verify on pin.

## DOCX, Word and preservation

- Microsoft Open XML package concepts: https://learn.microsoft.com/en-us/office/open-xml/general/how-to-create-a-package
- Word CustomXmlPart Office.js API: https://learn.microsoft.com/en-us/javascript/api/word/word.customxmlpart — exposed from WordApi 1.4, supports getXml()/adding custom parts in a Word add-in. Our main static teacher viewer **does not require an add-in**.
- Microsoft Office add-in docs on Custom XML Parts: https://github.com/OfficeDev/office-js-docs-pr/blob/main/docs/develop/support-for-task-pane-and-content-add-ins.md
- **Important distinction:** Microsoft says **inline custom XML markup** is removed from Word documents: https://support.microsoft.com/en-us/word/custom-xml-markup-in-word . InkWitness uses **OPC Custom XML Parts**, not inline markup.
- Word plain revision data is **not** a continuous replay of edit operations. OneDrive/SharePoint version histories generally belong to the cloud service and do not become a self-contained replay in an emailed DOCX.

## Browser APIs, hosting and license checks

- W3C Clipboard API: https://www.w3.org/TR/clipboard-apis/ — clipboard content/types may be observed when pasted, but source application is not reliably identified.
- MDN ClipboardEvent: https://developer.mozilla.org/en-US/docs/Web/API/ClipboardEvent/clipboardData
- MDN browser storage persistence and eviction: https://developer.mozilla.org/en-US/docs/Web/API/Storage_API/Storage_quotas_and_eviction_criteria ; https://developer.mozilla.org/en-US/docs/Web/API/StorageManager/persist
- GitHub Pages deploy from Actions: https://docs.github.com/en/pages/getting-started-with-github-pages/configuring-a-publishing-source-for-your-github-pages-site ; https://docs.github.com/en/get-started/start-your-journey/deploying-your-website-automatically
- `winkNLP`: https://github.com/winkjs/wink-nlp — MIT-licensed JavaScript NLP library that can run in browsers; model/data licenses must be checked separately.
- `jsdiff` license: https://github.com/components/jsdiff/blob/master/LICENSE.md — BSD terms; optional viewer presentation only, never substitute for semantic event causation.
- `Transformers.js`: https://github.com/huggingface/transformers.js — Apache-2.0 inference toolkit, optional/deferred; a model has its own license/size/accuracy constraints.

## School policy and ethical limitations

- **Ikast-Brande Gymnasium IB policies:** https://www.ikast-gym.dk/ib/more-about-the-ib/policies — Academic Integrity section covers independently produced work, proper sources, plagiarism including AI-generated materials, unauthorized assistance and excessive AI editing. **The Academic Integrity section says "Last reviewed: March 2025."** The `June 1st 2026` review date appears under the **Access and Inclusion** section, not Academic Integrity. Confirm with the school before deployment.
- The same school page documents **ExamCookie** for certain examinations. InkWitness is not a replacement for school-required examination monitoring.
- AI-text detection performance and false positives vary with setting, model and language; no generic “AI-authorship probability” is validated as a standalone disciplinary verdict. The request for an “MIT AI thing” has not identified a specific licensed, validated MIT tool or benchmark. **Do not invent that pedigree.**
- No accepted universal browser “AI deny” mechanism can guarantee that Claude, ChatGPT, Gemini or autonomous browser agents will refrain from operating a publicly accessible page. Public AI guidance is voluntary and complements clear student-facing rules.

## Research confidence / incomplete tests

The design-time research above supports a feasible **architecture**. It does **not** establish that Microsoft's desktop Word has preserved this particular evidence payload, that GitHub Pages WASM build succeeds under a repository prefix, that an agent-facing notice is honored, or that local browser storage is permanent. These are G0/G3 test requirements, not accomplished results.


## Verified branding / trademark references

- WordCraft browser HTML: https://github.com/storytold/wordcraft/blob/main/apps/wordcraft-web/index.html (title, Trunk-linked WordCraft favicon and loading text).
- WordCraft Trunk configuration: https://github.com/storytold/wordcraft/blob/main/apps/wordcraft-web/Trunk.toml (`public_url = "./"`; verify chosen tag and deployment base).
- ArtCraft trademark restriction: https://github.com/storytold/wordcraft/blob/main/docs/brand/LICENSE-brand.txt (marks are not open-source and must be removed from distributed modified forks; descriptive attribution allowed).
- WordCraft asset inventory: https://github.com/storytold/wordcraft/blob/main/ATTRIBUTION.md (original icon and other open assets).
- Branding implementation contract and original replacement artwork: `docs/BRANDING.md` and `assets/brand/`.


## Future V2 — Microsoft 365 references (not V1 dependencies)

Architecture and permissions to verify with the school during **V2-0**, not claims of tenant access:
- Microsoft identity for SPAs: https://learn.microsoft.com/en-us/entra/identity-platform/scenario-spa-overview
- Microsoft Graph Education: https://learn.microsoft.com/en-us/graph/education-concept-overview
- Education assignment resource: https://learn.microsoft.com/en-us/graph/api/resources/educationassignment
- Graph DriveItem (OneDrive/SharePoint): https://learn.microsoft.com/en-us/graph/api/resources/driveitem
- Graph permissions: https://learn.microsoft.com/en-us/graph/permissions-reference

Refer to `docs/V2_DISCOVERY_AND_SECURITY.md` for approval requirements. None of this is allowed to delay V1.
