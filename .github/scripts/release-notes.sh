#!/usr/bin/env bash
# Print the GitHub Release notes: one line per crate with its current version (from its
# Cargo.toml, +BUILD stripped) and a link to its CHANGELOG.md. The changelogs hold the
# details; the notes stay short on purpose (decided 2026-10-09).
#
# The links point at the files in the `v*` tag at HEAD (the release being made), so they
# show the changelog as released; without such a tag (a local run, CI), at `main`.
#
# Exits non-zero if a crate's CHANGELOG.md has no entries for its current version, so a
# release can't go out with an undocumented version.
#
# Usage: .github/scripts/release-notes.sh
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
repo_url="https://github.com/holgertkey/tagent"

crate_version() {
  sed -n 's/^version *= *"\(.*\)"/\1/p' "$root/$1/Cargo.toml" | head -n1
}

# has_entries CHANGELOG VERSION: true if a section for VERSION (any +BUILD) has an entry.
has_entries() {
  awk -v want="$2" '
    /^## \[/ {
      hdr = $0
      sub(/^## \[/, "", hdr); sub(/\].*$/, "", hdr); sub(/\+.*$/, "", hdr)
      in_section = (hdr == want)
      next
    }
    in_section && /^- / { found = 1; exit }
    END { exit !found }
  ' "$1"
}

ref="$(git -C "$root" tag --points-at HEAD --list 'v*' 2> /dev/null | sort -V | tail -n1 || true)"
[ -n "$ref" ] || ref=main

status=0
# tagent-cli first: it is the application most users download.
for crate in tagent-cli tagent-gui tagent; do
  version="$(crate_version "$crate")"
  version="${version%%+*}"
  if ! has_entries "$root/$crate/CHANGELOG.md" "$version"; then
    echo "error: $crate/CHANGELOG.md has no entries for version $version" >&2
    status=1
    continue
  fi
  printf -- '- **%s %s**: [changelog](%s/blob/%s/%s/CHANGELOG.md)\n' \
    "$crate" "$version" "$repo_url" "$ref" "$crate"
done

exit "$status"
