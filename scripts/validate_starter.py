#!/usr/bin/env python3
"""Validate the InkWitness repository starter ONLY.

This is intentionally NOT an application build or a DOCX interoperability test.
"""
from __future__ import annotations

import json
import pathlib
import sys
import struct
import xml.etree.ElementTree as ET
import zipfile

ROOT = pathlib.Path(__file__).resolve().parents[1]
# Synthetic DOCX test fixtures written by the WordCraft writer (no real student work). Each must
# name the synthetic author in its document text; any other .docx in the tree still fails.
SYNTHETIC_DOCX = {
    'vendor/wordcraft/crates/inkwitness/tests/fixtures/g0-evidence.docx',
}
SYNTHETIC_AUTHOR = 'Example Student'
REQUIRED = [
    'README.md', 'AGENTS.md', 'CLAUDE.md', 'LICENSE',
    'docs/SPEC.md', 'docs/AGENT_PROMPT.md', 'docs/UPSTREAM_PLAN.md',
    'docs/DOCX_FORMAT.md', 'docs/REPLAY_UI.md', 'docs/DIAGNOSTICS.md',
    'docs/POLICY_AND_PRIVACY.md', 'docs/STORAGE_AND_RECOVERY.md',
    'docs/GITHUB_PAGES.md', 'docs/BACKLOG.md', 'docs/TEST_MATRIX.md',
    'docs/SOURCES.md', 'docs/BRANDING.md',
    'docs/PROJECT_PHASES.md', 'docs/V2_MICROSOFT365_ROADMAP.md',
    'docs/V2_DISCOVERY_AND_SECURITY.md',
    'assets/brand/ATTRIBUTION.md', 'assets/brand/preview.html', 'assets/brand/brand-preview.png',
    'assets/brand/inkwitness-app-dark.svg', 'assets/brand/inkwitness-app-light.svg',
    'assets/brand/inkwitness-write.svg', 'assets/brand/inkwitness-review.svg',
    'assets/brand/inkwitness-lockup.svg', 'assets/brand/favicon.svg', 'assets/brand/favicon.ico',
    'assets/brand/png/inkwitness-16.png', 'assets/brand/png/inkwitness-32.png',
    'assets/brand/png/inkwitness-192.png', 'assets/brand/png/inkwitness-512.png',
    'schemas/evidence-v1.schema.json', 'schemas/assignment-v1.schema.json',
    'examples/tok-essay-assignment.json', 'examples/sample-evidence-v1.json',
    'examples/evidence-v1.2-unobserved.json', 'docs/adr/0003-sessions-and-unobserved-spans.md',
    'policies/ai-assistant-guidance.json', 'policies/STUDENT_NOTICE_TEMPLATE.md',
]

def load(name):
    with (ROOT / name).open(encoding='utf-8') as handle:
        return json.load(handle)


def main():
    missing = [name for name in REQUIRED if not (ROOT / name).is_file()]
    if missing:
        print('Missing starter files:', *missing, sep='\n  - ', file=sys.stderr)
        return 1
    schema1 = load('schemas/evidence-v1.schema.json')
    schema2 = load('schemas/assignment-v1.schema.json')
    evidence = load('examples/sample-evidence-v1.json')
    assignment = load('examples/tok-essay-assignment.json')
    load('policies/ai-assistant-guidance.json')
    try:
        import jsonschema
    except ImportError:
        print('jsonschema is not installed; only JSON parse and structural checks will run.')
        for label, data, schema in [('assignment', assignment, schema2), ('evidence', evidence, schema1)]:
            required_missing = set(schema['required']) - set(data)
            if required_missing:
                raise RuntimeError(f'{label} required keys missing: {required_missing}')
    else:
        jsonschema.Draft202012Validator.check_schema(schema1)
        jsonschema.Draft202012Validator.check_schema(schema2)
        fmt = jsonschema.FormatChecker()
        jsonschema.Draft202012Validator(schema1, format_checker=fmt).validate(evidence)
        jsonschema.Draft202012Validator(schema2, format_checker=fmt).validate(assignment)
        # Schema 1.2: the fixture with an unobserved span validates; 1.0 and 1.1 files still do,
        # and a file that claims 1.1 may not carry the 1.2 span list.
        v12 = jsonschema.Draft202012Validator(schema1, format_checker=fmt)
        fixture = load('examples/evidence-v1.2-unobserved.json')
        v12.validate(fixture)
        old = dict(fixture, schemaVersion='1.1')
        if v12.is_valid(old):
            raise AssertionError('A 1.1 file with unobservedSpans must not validate')
        del old['unobservedSpans']
        v12.validate(old)
        no_list = {k: v for k, v in fixture.items() if k != 'unobservedSpans'}
        if v12.is_valid(no_list):
            raise AssertionError('A 1.2 file must carry unobservedSpans (empty if nothing was missed)')
        print('Evidence 1.0, 1.1 and 1.2 (with unobserved spans) VALID.')
        print('JSON examples VALID against draft-2020-12 schemas.')

    if evidence['attemptId'] != evidence['events'][0]['attemptId']:
        raise AssertionError('Example event attemptId must match sample evidence attemptId')
    if evidence['assignmentId'] != assignment['assignmentId']:
        raise AssertionError('Example assignmentId mismatch')
    for p in ROOT.rglob('*'):
        if not p.is_file():
            continue
        if p.suffix.lower() in ('.docx', '.docm', '.walproject'):
            if p.relative_to(ROOT).as_posix() in SYNTHETIC_DOCX:
                with zipfile.ZipFile(p) as z:
                    text = z.read('word/document.xml').decode('utf-8')
                if SYNTHETIC_AUTHOR not in text:
                    raise AssertionError(f'Synthetic fixture does not name {SYNTHETIC_AUTHOR}: {p}')
                continue
            raise AssertionError(f'No real student document in starter: {p}')
    for path in (ROOT / 'assets' / 'brand').glob('*.svg'):
        document = ET.parse(path)
        if not document.getroot().tag.endswith('svg'):
            raise AssertionError(f'Not SVG: {path}')
    for sz in (16, 32, 192, 512):
        path = ROOT / 'assets' / 'brand' / 'png' / f'inkwitness-{sz}.png'
        raw = path.read_bytes()
        if raw[:8] != b'\x89PNG\r\n\x1a\n':
            raise AssertionError(f'Bad PNG signature: {path}')
        width, height = struct.unpack('>II', raw[16:24])
        if (width, height) != (sz, sz):
            raise AssertionError(f'Incorrect icon dimensions: {path}: {width}x{height}')
    ico = (ROOT / 'assets' / 'brand' / 'favicon.ico').read_bytes()
    if len(ico) < 30 or ico[:4] != b'\x00\x00\x01\x00':
        raise AssertionError('Invalid favicon ICO header')
    if 'InkWitness' not in (ROOT / 'README.md').read_text(encoding='utf-8'):
        raise AssertionError('README branding missing')
    phases = (ROOT / 'docs/PROJECT_PHASES.md').read_text(encoding='utf-8')
    roadmap = (ROOT / 'docs/V2_MICROSOFT365_ROADMAP.md').read_text(encoding='utf-8')
    agent = (ROOT / 'AGENTS.md').read_text(encoding='utf-8')
    if not all(x in phases for x in ('V1', 'V2', 'G0', 'G3')) or not all(x in roadmap for x in ('OneDrive', 'SharePoint', 'Teams')):
        raise AssertionError('V1/V2 separation or Microsoft 365 roadmap missing')
    if 'V1 only' not in agent or 'V2 SCOPE LOCK' not in agent:
        raise AssertionError('Coding-agent V1 scope lock missing')
    print(f'OK: {len(REQUIRED)} required files present, SVG/XML and PNG/ICO branding valid. Sample data is synthetic.')
    print('NOT TESTED: WordCraft WASM build, DOCX export, embedded Custom XML, Microsoft Word compatibility, browser recovery or playback.')
    return 0


if __name__ == '__main__':
    sys.exit(main())
