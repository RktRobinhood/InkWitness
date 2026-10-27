# GitHub Pages deployment and release checklist

## Public by design, private writing by default

The app's source and compiled WASM are public. `write` and `review` are **static URLs**. Neither site uploads student DOCX content to the static host in the intended MVP. A direct, unpublished reviewer URL is not a login. The reviewer must process selected files locally; never commit real student submissions or local storage dumps.

Example project URL after publishing:

```text
https://<github-owner>.github.io/<repo>/write/
https://<github-owner>.github.io/<repo>/review/
```

The root `/` may show a neutral project home. WordCraft's bundle and its `.wasm` must resolve correctly under the `/<repo>/` prefix, **not from `/`**. Use explicit relative URLs/base-path configuration. The same origin helps browser storage persist predictably; changing the origin creates a different storage namespace.

## Build and CI strategy

1. Finish G0 proof on localhost (`127.0.0.1` secure context). Pin Rust, Trunk and relevant JS tools in lockfiles. Keep upstream licenses and attribution.
2. Build actual WordCraft WebAssembly bundle(s). Inspect that `write` and `review` each start, that there are no remote source-map URLs, and that MIME type `application/wasm` is honored.
3. Generate an output folder like `dist/write/index.html`, `dist/review/index.html`, plus asset subdirectories. Reuse common static assets where possible and avoid duplicate huge `.wasm` packages if modes can share a build.
4. Add a Pages Actions workflow with official `configure-pages`, `upload-pages-artifact`, and `deploy-pages` actions; run on `main` **only after release gates pass**. The provided repo intentionally contains no auto-deploy workflow while there is no working app.
5. In Settings → Pages, select **GitHub Actions** as deployment source, and check the published paths on actual Windows/Mac school browsers. Verify CDN and worker paths do not reference localhost.
6. Disable third-party analytics, public telemetry endpoints and external clipboard/site callbacks. A strict CSP can block network `connect-src` by default, but must be tested with the actual WASM/worker/font assets. Network inspection should show **no essays or replay logs being transmitted**.
7. Document a safe update path that does not silently discard IndexedDB drafts: schema migration, stable origin, backup/export warning and test across the previous build.

A workflow for spec validation alone is at `.github/workflows/spec-check.yml`. Do not treat its green check as proof of application readiness.

## Deployment smoke checks

- Fresh `/write/` displays mandatory student identity + academic-integrity disclosure, then the real editor.
- First manual DOCX export requires checked declaration; browser autosave works before it.
- DOCX exports from actual app (not a mock) and opens in Word.
- Fresh `/review/` has file picker; no DOCX HTTP upload; real read-only WordCraft renderer loads file and plays evidence.
- Accurate exact paste pin and smooth scroll under normal/reduced-motion settings.
- Offline after first load is **not** guaranteed unless a service worker/offline cache has been intentionally implemented and tested. Do not claim this property by default.
- Browser and GitHub Action logs contain no sample real student names/essays.

## School policies

A teacher should confirm the school approves student writing/process telemetry and local device retention under GDPR and that the intended activity is **not** an official exam mode requiring separate monitoring. The project includes student-visible disclosure but does not itself establish institutional compliance.


## InkWitness branding on GitHub Pages

Use the committed `assets/brand/` originals for both routes. Relative references to the icon/favicons **must be generated into** the built artifact or copied to that artifact with correct paths; do not assume `../../assets/brand/` resolves from the final deployed site. `apps/wordcraft-web/index.html` in upstream has a Trunk-linked favicon to the WordCraft icon; replace it in the pinned source and verify in `/write/` and `/review/`. The public repository's social-preview image is at `assets/brand/social-preview.png`; its use as the GitHub social preview requires a manual upload in GitHub repository settings. A self-contained brand gallery is available at `assets/brand/preview.html` but is not a functional classroom tool.
