#!/usr/bin/env bash
# Agent-run helper; NOT executed when creating this starter package.
# Imports the complete WordCraft workspace under vendor/wordcraft as an upstream subtree.
set -euo pipefail

if [[ ! -d .git ]]; then
  echo 'Run at the root of an initialized InkWitness Git repository.' >&2
  exit 2
fi
if [[ -n "$(git status --porcelain)" ]]; then
  echo 'Commit or stash the starter files before adding the subtree.' >&2
  exit 2
fi
if [[ -e vendor/wordcraft ]]; then
  echo 'vendor/wordcraft already exists; inspect and do not overwrite it.' >&2
  exit 2
fi
upstream='https://github.com/storytold/wordcraft.git'
tag='v0.11.0'

if ! git remote get-url wordcraft-upstream >/dev/null 2>&1; then
  git remote add wordcraft-upstream "$upstream"
fi
git fetch wordcraft-upstream "refs/tags/${tag}:refs/tags/${tag}"
commit="$(git rev-parse "${tag}^{commit}")"
echo "Verified tag ${tag}: ${commit}"
# Inspect upstream licenses and local dependencies before running builds.
git subtree add --prefix=vendor/wordcraft wordcraft-upstream "$tag" --squash
printf 'Upstream URL: %s\nPinned tag: %s\nResolved commit: %s\nImported on: %s\n' \
  "$upstream" "$tag" "$commit" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" > docs/UPSTREAM_LOCK.md
printf '\nSource imported. Review docs/UPSTREAM_LOCK.md and commit it.\n'
