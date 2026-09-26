#!/usr/bin/env bash
# Print the GitHub Release notes for the versions currently in the three crates'
# Cargo.toml files, assembled from each crate's own CHANGELOG.md.
#
# Which changelog sections are included:
#   - tagent-cli, tagent-gui: every section whose header version equals the crate's
#     current version V once any "+BUILD" suffix is stripped -- "## [0.16.0]",
#     "## [0.16.0+017]", "## [0.16.0+016]", ... Within a release cycle only their
#     +BUILD counter moves, so that is everything since the last release.
#   - tagent: its plain version can move more than once between releases (0.18.2 and
#     0.18.3 both unpublished, say), so every section whose version is greater than the
#     tagent version of the previous release and at most the current one. The previous
#     release is the highest `v*` tag (by version) that doesn't point at HEAD -- the tag
#     being released right now is skipped -- and its tagent version is read from
#     `git show <tag>:tagent/Cargo.toml`. With no such tag, or a tag that predates the
#     tagent crate, every section up to the current version is included. If tagent's
#     version didn't change since that release, the sections equal to the current version
#     are used, as for the apps.
# The entries are merged under one "### Added" / "### Changed" / ... heading each, so
# dev-iteration sections don't repeat headings.
#
# Exits non-zero if a crate has no changelog section for its current version, so a
# release can't go out with empty notes, and in a shallow git clone (no tags to find
# the previous release by; check out with `fetch-depth: 0`).
#
# Usage: .github/scripts/release-notes.sh
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
repo_url="https://github.com/holgertkey/tagent"

strip_build() { printf '%s' "${1%%+*}"; }

crate_version() {
  sed -n 's/^version *= *"\(.*\)"/\1/p' "$root/$1/Cargo.toml" | head -n1
}

# version_le A B: true if A <= B.
version_le() {
  [ "$(printf '%s\n%s\n' "$1" "$2" | sort -V | head -n1)" = "$1" ]
}

# changelog_versions CHANGELOG: every distinct header version, +BUILD stripped.
changelog_versions() {
  sed -n 's/^## \[\([0-9][^]+]*\)[]+].*/\1/p' "$1" | sort -uV
}

# previous_tagent_version: tagent's version in the previous release, or nothing.
previous_tagent_version() {
  git -C "$root" rev-parse --git-dir > /dev/null 2>&1 || return 0
  if [ "$(git -C "$root" rev-parse --is-shallow-repository)" = "true" ]; then
    echo "error: shallow git clone; the previous release's tag can't be found" \
         "(check out with fetch-depth: 0)" >&2
    return 1
  fi
  local at_head tag
  at_head="$(git -C "$root" tag --points-at HEAD --list 'v*')"
  tag="$(git -C "$root" tag --list 'v*' --sort=-v:refname \
         | grep -vxF -e "$at_head" -e '' | head -n1 || true)"
  [ -n "$tag" ] || return 0
  # A tag older than the tagent crate has no tagent/Cargo.toml: no previous release.
  git -C "$root" show "$tag:tagent/Cargo.toml" 2> /dev/null \
    | sed -n 's/^version *= *"\(.*\)"/\1/p' | head -n1 || true
}

# section_notes CHANGELOG VERSION...: merged entries of the sections for these versions.
section_notes() {
  local changelog="$1"; shift
  awk -v want="$*" '
    BEGIN { n_want = split(want, w, " "); for (i = 1; i <= n_want; i++) wanted[w[i]] = 1 }
    /^## \[/ {
      hdr = $0
      sub(/^## \[/, "", hdr); sub(/\].*$/, "", hdr); sub(/\+.*$/, "", hdr)
      in_section = (hdr in wanted); cat = ""
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
  ' "$changelog"
}

status=0
out=""
# tagent-cli first: it is the application most users download.
for crate in tagent-cli tagent-gui tagent; do
  version="$(strip_build "$(crate_version "$crate")")"
  changelog="$root/$crate/CHANGELOG.md"

  # The current version itself must have entries, whatever else is included.
  if [ -z "$(section_notes "$changelog" "$version")" ]; then
    echo "error: $crate/CHANGELOG.md has no entries for version $version" >&2
    status=1
    continue
  fi

  versions="$version"
  if [ "$crate" = tagent ]; then
    previous="$(previous_tagent_version)" || { status=1; continue; }
    previous="$(strip_build "$previous")"
    if [ "$previous" != "$version" ]; then
      versions=""
      for v in $(changelog_versions "$changelog"); do
        version_le "$v" "$version" || continue
        if [ -n "$previous" ] && version_le "$v" "$previous"; then continue; fi
        versions+="$v "
      done
    fi
  fi

  notes="$(section_notes "$changelog" $versions)"
  out+="$(printf '## %s %s\n\n%s\n\n' "$crate" "$version" "$notes")"$'\n\n'
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
