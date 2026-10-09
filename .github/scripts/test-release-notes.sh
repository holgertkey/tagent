#!/usr/bin/env bash
# Regression test for release-notes.sh: builds throwaway git repositories with fixture
# Cargo.toml/CHANGELOG.md files for the three crates, runs the script in them and checks
# the notes. Never touches the real repository.
#
# Usage: .github/scripts/test-release-notes.sh
set -euo pipefail

script="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/release-notes.sh"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

failures=0
fail() { echo "FAIL: $*" >&2; failures=$((failures + 1)); }

g() {
  git -c user.name=test -c user.email=test@example.invalid \
      -c commit.gpgsign=false -c tag.gpgsign=false "$@"
}

# write_crate DIR CRATE VERSION SECTION...: Cargo.toml plus a changelog whose sections
# (newest first) each hold one entry.
write_crate() {
  local dir="$1" crate="$2" version="$3"; shift 3
  mkdir -p "$dir/$crate"
  printf '[package]\nname = "%s"\nversion = "%s"\n' "$crate" "$version" > "$dir/$crate/Cargo.toml"
  {
    printf '# Changelog\n\n## [Unreleased]\n\n'
    for v in "$@"; do
      printf '## [%s] - 2026-01-01\n\n### Added\n- entry-%s-%s\n\n' "$v" "$crate" "$v"
    done
  } > "$dir/$crate/CHANGELOG.md"
}

# new_repo NAME: a git repo with the script and the three crates' fixtures, one commit.
new_repo() {
  local dir="$tmp/$1"
  mkdir -p "$dir/.github/scripts"
  cp "$script" "$dir/.github/scripts/"
  write_crate "$dir" tagent-cli 1.0.0 1.0.0 0.9.0
  write_crate "$dir" tagent-gui 2.0.0 2.0.0+002 1.9.0
  write_crate "$dir" tagent 0.19.0+003 0.19.0 0.18.1
  g -C "$dir" init -q -b main
  g -C "$dir" add -A && g -C "$dir" commit -q -m base
  printf '%s' "$dir"
}

# run_notes DIR: the script's stdout; its exit status goes to $rc.
run_notes() { rc=0; notes="$("$1/.github/scripts/release-notes.sh" 2> "$tmp/stderr")" || rc=$?; }

has()     { grep -qF -- "$2" <<< "$notes" || fail "$1: expected '$2' in the notes"; }
has_not() { ! grep -qF -- "$2" <<< "$notes" || fail "$1: did not expect '$2' in the notes"; }

url=https://github.com/holgertkey/tagent/blob

# 1. A tag at HEAD: one line per crate, +BUILD stripped, links at the tag, no entries.
t=tagged
dir="$(new_repo $t)"
g -C "$dir" tag v1.0.0
run_notes "$dir"
[ "$rc" = 0 ] || fail "$t: exit $rc: $(cat "$tmp/stderr")"
has $t "- **tagent-cli 1.0.0**: [changelog]($url/v1.0.0/tagent-cli/CHANGELOG.md)"
has $t "- **tagent-gui 2.0.0**: [changelog]($url/v1.0.0/tagent-gui/CHANGELOG.md)"
has $t "- **tagent 0.19.0**: [changelog]($url/v1.0.0/tagent/CHANGELOG.md)"
has_not $t "entry-"
[ "$(wc -l <<< "$notes")" = 3 ] || fail "$t: expected 3 lines, got: $notes"

# 2. No tag at HEAD (an older one doesn't count): links at main.
t=untagged
dir="$(new_repo $t)"
g -C "$dir" tag v0.9.0
g -C "$dir" commit -q --allow-empty -m next
run_notes "$dir"
[ "$rc" = 0 ] || fail "$t: exit $rc: $(cat "$tmp/stderr")"
has $t "$url/main/tagent-cli/CHANGELOG.md"
has_not $t "v0.9.0"

# 3. The current version has no entries (only an older section): fail.
t=missing-current
dir="$(new_repo $t)"
write_crate "$dir" tagent 0.20.0 0.19.0
run_notes "$dir"
[ "$rc" != 0 ] || fail "$t: expected a non-zero exit"
grep -qF "tagent/CHANGELOG.md has no entries for version 0.20.0" "$tmp/stderr" \
  || fail "$t: missing error message, got: $(cat "$tmp/stderr")"

# 4. A section header without entries doesn't count either.
t=empty-section
dir="$(new_repo $t)"
printf '# Changelog\n\n## [1.0.0] - 2026-01-01\n\n### Added\n\n## [0.9.0]\n\n- old\n' \
  > "$dir/tagent-cli/CHANGELOG.md"
run_notes "$dir"
[ "$rc" != 0 ] || fail "$t: expected a non-zero exit"
grep -qF "tagent-cli/CHANGELOG.md has no entries for version 1.0.0" "$tmp/stderr" \
  || fail "$t: missing error message, got: $(cat "$tmp/stderr")"

if [ "$failures" -gt 0 ]; then
  echo "$failures failure(s)" >&2
  exit 1
fi
echo "release-notes.sh: all tests passed"
