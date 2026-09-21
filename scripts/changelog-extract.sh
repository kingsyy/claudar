#!/usr/bin/env bash
# Print the CHANGELOG.md section for one version.
#
# Used by the release workflow to fill both the GitHub release body and the
# updater's `latest.json` notes, so the in-app "What's new" text and the release
# page never drift apart.
#
#   scripts/changelog-extract.sh 0.4.6          # or v0.4.6
#
# Exits non-zero if the version has no section, so a release can't silently ship
# with empty notes.
set -euo pipefail

version="${1:?usage: changelog-extract.sh <version>}"
version="${version#v}"
changelog="$(dirname "$0")/../CHANGELOG.md"

# Print the lines after the matching "## [x.y.z]" heading, stopping at the next
# "## ". Blank lines are buffered rather than printed immediately so trailing
# blanks before the next heading get dropped (no `tac`/`sed` trim -- `tac` does
# not exist on macOS).
section=$(awk -v want="$version" '
  /^## / {
    if (found) exit
    # Match "## [0.4.6] - date" and "## [0.4.6]".
    if (index($0, "[" want "]") > 0) { found = 1; next }
    next
  }
  found {
    if ($0 ~ /^[[:space:]]*$/) { pending = pending "\n"; next }
    if (started) printf "%s", pending
    pending = ""
    started = 1
    print
  }
' "$changelog")

if [ -z "$section" ]; then
  echo "error: no CHANGELOG.md section found for version $version" >&2
  exit 1
fi

printf '%s\n' "$section"
