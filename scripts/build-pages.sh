#!/usr/bin/env bash
# Builds the static GitHub Pages site:
#   /           project home (site/index.html, images from site/img/)
#   /write/     InkWitness Write  } both load one shared WASM bundle from /app/
#   /review/    InkWitness Review }
#   /brand/     InkWitness favicons and logos
#   /licenses/  InkWitness and upstream WordCraft license and attribution files
#
# Needs the toolchain upstream WordCraft pins (Rust 1.95, wasm32-unknown-unknown) and Trunk.
# Usage: scripts/build-pages.sh [output-dir]   (default: dist/pages)
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
out="${1:-$root/dist/pages}"
web="$root/vendor/wordcraft/apps/wordcraft-web"
wordcraft="$root/vendor/wordcraft"

rm -rf "$out"
mkdir -p "$out"
out="$(cd "$out" && pwd)"

# One release build. Asset URLs are emitted as ../app/<file>, which resolves from /write/ and /review/.
(cd "$web" && trunk build --release --public-url ../app/ --dist "$out/app")

for route in write review; do
  mkdir -p "$out/$route"
  cp "$out/app/index.html" "$out/$route/index.html"
done
rm "$out/app/index.html"
rm -f "$out/app/.trunk-lock"

cp "$root/site/index.html" "$out/index.html"
mkdir -p "$out/img"
cp "$root"/site/img/* "$out/img/"

mkdir -p "$out/brand"
cp "$root/assets/brand/favicon.svg" "$root/assets/brand/favicon.ico" "$root/assets/brand/apple-touch-icon.png" \
   "$root/assets/brand/inkwitness-lockup.svg" "$root/assets/brand/inkwitness-write.svg" \
   "$root/assets/brand/inkwitness-review.svg" "$out/brand/"

mkdir -p "$out/licenses"
cp "$root/LICENSE" "$out/licenses/InkWitness-LICENSE.txt"
cp "$root/assets/brand/ATTRIBUTION.md" "$out/licenses/InkWitness-brand-ATTRIBUTION.md"
cp "$wordcraft/LICENSE-MIT" "$out/licenses/WordCraft-LICENSE-MIT.txt"
cp "$wordcraft/LICENSE-APACHE" "$out/licenses/WordCraft-LICENSE-APACHE.txt"
cp "$wordcraft/NOTICE" "$out/licenses/WordCraft-NOTICE.txt"
cp "$wordcraft/ATTRIBUTION.md" "$out/licenses/WordCraft-ATTRIBUTION.md"
for ofl in "$wordcraft"/assets/fonts/OFL-*.txt; do
  cp "$ofl" "$out/licenses/"
done
{
  echo '<!doctype html><html lang="en"><head><meta charset="utf-8"><title>InkWitness — licenses and notices</title></head><body>'
  echo '<h1>Licenses and notices</h1>'
  echo '<p>InkWitness is built on WordCraft by the ArtCraft team and the WordCraft contributors. It is independently developed and is not affiliated with or endorsed by the ArtCraft Team or Microsoft.</p><ul>'
  for f in "$out"/licenses/*; do
    name="$(basename "$f")"
    [[ "$name" == index.html ]] && continue
    echo "<li><a href=\"$name\">$name</a></li>"
  done
  echo '</ul><p><a href="../">InkWitness home</a></p></body></html>'
} > "$out/licenses/index.html"

touch "$out/.nojekyll"

# Every ../app/ asset the route pages reference must exist in the bundle.
missing=0
for ref in $(grep -oE '\.\./app/[A-Za-z0-9._-]+' "$out/write/index.html" | sort -u); do
  if [[ ! -f "$out/${ref#../}" ]]; then
    echo "missing asset: $ref" >&2
    missing=1
  fi
done
if ! ls "$out"/app/*.wasm >/dev/null 2>&1; then
  echo 'no .wasm emitted' >&2
  missing=1
fi
# The distributed fork must not ship ArtCraft trademark assets (docs/brand/LICENSE-brand.txt).
if grep -rliE 'artcraft-(logo|mark)' "$out/index.html" "$out/app" "$out/write" "$out/review" "$out/brand" >/dev/null; then
  echo 'ArtCraft mark referenced in build output' >&2
  missing=1
fi
[[ "$missing" == 0 ]]

echo "Built $out"
du -sh "$out"/app/*.wasm "$out"
