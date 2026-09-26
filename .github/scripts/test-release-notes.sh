#!/usr/bin/env bash
# Regression test for release-notes.sh: builds throwaway git repositories with fixture
# Cargo.toml/CHANGELOG.md files for the three crates, runs the script in them and checks
# which changelog sections end up in the notes. Never touches the real repository.
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
# (newest first) each hold one entry with a unique marker, "entry-<crate>-<version>".
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

# new_repo NAME: a git repo with the script and the two apps' fixtures, one commit.
new_repo() {
  local dir="$tmp/$1"
  mkdir -p "$dir/.github/scripts"
  cp "$script" "$dir/.github/scripts/"
  write_crate "$dir" tagent-cli 1.0.0 1.0.0+002 1.0.0+001 0.9.0
  write_crate "$dir" tagent-gui 2.0.0 2.0.0 1.9.0
  g -C "$dir" init -q -b main
  printf '%s' "$dir"
}

commit() { g -C "$1" add -A && g -C "$1" commit -q -m "$2"; }

# run_notes DIR: the script's stdout; its exit status goes to $rc.
run_notes() { rc=0; notes="$("$1/.github/scripts/release-notes.sh" 2> "$tmp/stderr")" || rc=$?; }

has()     { grep -qF -- "$2" <<< "$notes" || fail "$1: expected '$2' in the notes"; }
has_not() { ! grep -qF -- "$2" <<< "$notes" || fail "$1: did not expect '$2' in the notes"; }

# 1. Unpublished intermediate tagent versions are collected; the tag at HEAD (the
#    release being made) is skipped when looking for the previous release.
t=range
dir="$(new_repo $t)"
write_crate "$dir" tagent 0.18.1 0.18.1 0.18.0
commit "$dir" base && g -C "$dir" tag v1.0.0
write_crate "$dir" tagent 0.19.0 0.19.0 0.18.3+002 0.18.2 0.18.1 0.18.0
commit "$dir" bump && g -C "$dir" tag v1.1.0
run_notes "$dir"
[ "$rc" = 0 ] || fail "$t: exit $rc: $(cat "$tmp/stderr")"
has $t "## tagent 0.19.0"
has $t "entry-tagent-0.19.0"
has $t "entry-tagent-0.18.3+002"
has $t "entry-tagent-0.18.2"
has_not $t "entry-tagent-0.18.1"
has_not $t "entry-tagent-0.18.0"
# The apps keep matching their current version only.
has $t "entry-tagent-cli-1.0.0+002"
has $t "entry-tagent-cli-1.0.0+001"
has_not $t "entry-tagent-cli-0.9.0"
has $t "entry-tagent-gui-2.0.0"
has_not $t "entry-tagent-gui-1.9.0"

# 2. No previous release tag: every section up to the current version.
t=no-tag
dir="$(new_repo $t)"
write_crate "$dir" tagent 0.18.1 0.18.1 0.18.0
commit "$dir" base
run_notes "$dir"
[ "$rc" = 0 ] || fail "$t: exit $rc: $(cat "$tmp/stderr")"
has $t "entry-tagent-0.18.1"
has $t "entry-tagent-0.18.0"

# 3. A previous tag that predates the tagent crate counts as no previous release.
t=tag-before-tagent
dir="$(new_repo $t)"
commit "$dir" base && g -C "$dir" tag v0.9.0
write_crate "$dir" tagent 0.18.1 0.18.1 0.18.0
commit "$dir" add-tagent
run_notes "$dir"
[ "$rc" = 0 ] || fail "$t: exit $rc: $(cat "$tmp/stderr")"
has $t "entry-tagent-0.18.1"
has $t "entry-tagent-0.18.0"

# 4. tagent unchanged since the previous release: its current section, as for the apps.
t=unchanged
dir="$(new_repo $t)"
write_crate "$dir" tagent 0.18.1 0.18.1 0.18.0
commit "$dir" base && g -C "$dir" tag v1.0.0
write_crate "$dir" tagent-cli 1.1.0 1.1.0 1.0.0
commit "$dir" app-only
run_notes "$dir"
[ "$rc" = 0 ] || fail "$t: exit $rc: $(cat "$tmp/stderr")"
has $t "entry-tagent-0.18.1"
has_not $t "entry-tagent-0.18.0"

# 5. Versions compare numerically (0.10.0 > 0.9.0), not as strings.
t=numeric
dir="$(new_repo $t)"
write_crate "$dir" tagent 0.9.0 0.9.0
commit "$dir" base && g -C "$dir" tag v1.0.0
write_crate "$dir" tagent 0.10.0 0.10.0 0.9.1 0.9.0
commit "$dir" bump
run_notes "$dir"
[ "$rc" = 0 ] || fail "$t: exit $rc: $(cat "$tmp/stderr")"
has $t "entry-tagent-0.10.0"
has $t "entry-tagent-0.9.1"
has_not $t "entry-tagent-0.9.0"

# 6. The current tagent version has no section: fail, even though older unpublished
#    sections exist.
t=missing-current
dir="$(new_repo $t)"
write_crate "$dir" tagent 0.18.1 0.18.1
commit "$dir" base && g -C "$dir" tag v1.0.0
write_crate "$dir" tagent 0.18.2 0.18.2 0.18.1
sed -i 's/^version = "0.18.2"/version = "0.19.0"/' "$dir/tagent/Cargo.toml"
commit "$dir" bump
run_notes "$dir"
[ "$rc" != 0 ] || fail "$t: expected a non-zero exit"
grep -qF "tagent/CHANGELOG.md has no entries for version 0.19.0" "$tmp/stderr" \
  || fail "$t: missing error message, got: $(cat "$tmp/stderr")"

# 7. A shallow clone can't see the previous release: fail loudly.
t=shallow
src="$tmp/range"
g clone -q --depth 1 "file://$src" "$tmp/$t"
run_notes "$tmp/$t"
[ "$rc" != 0 ] || fail "$t: expected a non-zero exit"
grep -qF "shallow git clone" "$tmp/stderr" || fail "$t: missing error message, got: $(cat "$tmp/stderr")"

if [ "$failures" -gt 0 ]; then
  echo "$failures failure(s)" >&2
  exit 1
fi
echo "release-notes.sh: all tests passed"
