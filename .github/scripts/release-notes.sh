#!/usr/bin/env bash
# Print the GitHub Release notes for the versions currently in the three crates'
# Cargo.toml files, assembled from each crate's own CHANGELOG.md.
#
# For a crate at version V, every changelog section whose header version equals V once
# any "+BUILD" suffix is stripped is included -- "## [0.16.0]", "## [0.16.0+017]",
# "## [0.16.0+016]", ... -- and their entries are merged under one "### Added" /
# "### Changed" / ... heading each, so dev-iteration sections don't repeat headings.
#
# Exits non-zero if a crate has no changelog section for its current version, so a
# release can't go out with empty notes.
#
# Usage: .github/scripts/release-notes.sh
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
repo_url="https://github.com/holgertkey/tagent"

crate_version() {
  sed -n 's/^version *= *"\(.*\)"/\1/p' "$root/$1/Cargo.toml" | head -n1
}

# section_notes CHANGELOG VERSION
section_notes() {
  awk -v want="${2%%+*}" '
    /^## \[/ {
      hdr = $0
      sub(/^## \[/, "", hdr); sub(/\].*$/, "", hdr); sub(/\+.*$/, "", hdr)
      in_section = (hdr == want); cat = ""
      next
    }
    /^---[[:space:]]*$/ { in_section = 0; next }
    !in_section { next }
    /^### / {
      cat = $0; sub(/^### /, "", cat)
      if (!(cat in seen)) { seen[cat] = 1; order[++n] = cat }
      next
    }
    cat != "" { body[cat] = body[cat] $0 "\n" }
    END {
      for (i = 1; i <= n; i++) {
        text = body[order[i]]
        gsub(/^\n+/, "", text); gsub(/\n+$/, "", text)   # trim blank edges
        if (text != "") printf "### %s\n\n%s\n\n", order[i], text
      }
    }
  ' "$1"
}

status=0
out=""
# tagent-cli first: it is the application most users download.
for crate in tagent-cli tagent-gui tagent; do
  version="$(crate_version "$crate")"
  notes="$(section_notes "$root/$crate/CHANGELOG.md" "$version")"
  if [ -z "$notes" ]; then
    echo "error: $crate/CHANGELOG.md has no entries for version ${version%%+*}" >&2
    status=1
    continue
  fi
  out+="$(printf '## %s %s\n\n%s\n\n' "$crate" "${version%%+*}" "$notes")"$'\n\n'
done

printf '%s' "$out"
printf 'Full history: [tagent-cli](%s/blob/main/tagent-cli/CHANGELOG.md), [tagent-gui](%s/blob/main/tagent-gui/CHANGELOG.md), [tagent](%s/blob/main/tagent/CHANGELOG.md).\n' \
  "$repo_url" "$repo_url" "$repo_url"

# GitHub rejects a release body over 125000 characters; fail here rather than at upload.
if [ "${#out}" -gt 120000 ]; then
  echo "error: release notes are ${#out} characters, over GitHub's 125000 limit;" \
       "consolidate the +BUILD changelog sections first" >&2
  status=1
fi

exit "$status"
