#!/usr/bin/env bash
# Publish the workspace crates to crates.io, in dependency order, skipping any crate
# whose current version is already there (the three crates are versioned independently,
# so a release often changes only some of them).
#
# Usage: .github/scripts/publish-crates.sh [--dry-run] [extra `cargo publish` args]
#
# Real runs need CARGO_REGISTRY_TOKEN in the environment and refuse a version that still
# carries a "+BUILD" suffix (release versions are plain MAJOR.MINOR.PATCH). A dry run
# packages and verifies everything without either requirement.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
crates=(tagent tagent-cli tagent-gui)
dry_run=()
if [ "${1:-}" = "--dry-run" ]; then
  dry_run=(--dry-run)
  shift
fi

crate_version() {
  sed -n 's/^version *= *"\(.*\)"/\1/p' "$root/$1/Cargo.toml" | head -n1
}

# 0 = already on crates.io, 1 = not there. Anything else is an error, not "publish it".
is_published() {
  local code
  code="$(curl -s -o /dev/null -w '%{http_code}' \
    -A 'tagent-release-script (https://github.com/holgertkey/tagent)' \
    "https://crates.io/api/v1/crates/$1/$2")"
  case "$code" in
    200) return 0 ;;
    404) return 1 ;;
    *) echo "error: crates.io lookup for $1 $2 returned HTTP $code" >&2; exit 2 ;;
  esac
}

exclude=()
pending=0
for crate in "${crates[@]}"; do
  version="$(crate_version "$crate")"
  if [ ${#dry_run[@]} -eq 0 ] && [[ "$version" == *+* ]]; then
    echo "error: $crate is at $version; strip the +BUILD suffix before releasing" >&2
    exit 1
  fi
  if is_published "$crate" "$version"; then
    echo "$crate $version is already on crates.io; skipping"
    exclude+=(--exclude "$crate")
  else
    echo "$crate $version will be published"
    pending=$((pending + 1))
  fi
done

if [ "$pending" -eq 0 ]; then
  echo "nothing to publish"
  exit 0
fi

cd "$root"
# --workspace publishes in dependency order and verifies dependents against the local
# packages, so tagent-cli/tagent-gui can go out in the same run as the tagent they need.
cargo publish --workspace --locked "${exclude[@]}" "${dry_run[@]}" "$@"
