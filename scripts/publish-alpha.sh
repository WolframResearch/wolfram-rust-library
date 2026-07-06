#!/usr/bin/env bash
# Publish the workspace's publishable crates to crates.io.
#
# The version lives in each crate's Cargo.toml — this script never edits it and
# has no copy of its own. cargo enforces version consistency at publish time via
# the `=X.Y.Z` internal-dep pins, so there is nothing here to keep in sync.
#
# Modes:
#   bash scripts/publish-alpha.sh                   # dry-run (default)
#   bash scripts/publish-alpha.sh --publish         # publish to crates.io + tag
#   bash scripts/publish-alpha.sh --check-only      # just `cargo check --workspace`
#   bash scripts/publish-alpha.sh --resume <crate>  # publish from <crate> onward
#
# Prerequisites:
#   - cargo present.
#   - Run on a Wolfram-equipped machine: the -sys crates need the Wolfram C
#     headers (found via wolfram-app-discovery) when cargo packages them.
#   - `cargo login` with a token that owns every crate in PUBLISH. New crates
#     need an owner added first: cargo owner --add <user> <crate>
#   - For --publish: a clean working tree (the script bails otherwise).

set -euo pipefail

# The publishable crates. cargo figures out the topological publish order itself
# and resolves their inter-dependencies locally, so a single `cargo publish`
# over the whole set validates even though the new versions aren't on crates.io
# yet. (The other workspace members — examples, xtask — are not published.)
# Entries are cargo package names (`-p <name>`), not directory names — that's
# why `wolfram-cli`'s entry is `cargo-wl`, its `[package] name`.
PUBLISH=(
  wolfram-app-discovery
  wolfram-serialize-macros
  wolfram-export-macros
  wolfram-serialize
  wolfram-expr
  wolfram-export-core
  wstp-sys
  wolfram-library-link-sys
  wstp
  wolfram-library-link
  wolfram-export
  cargo-wl
)

MODE="dry-run"
RESUME_FROM=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --dry-run)    MODE="dry-run"; shift ;;
    --publish)    MODE="publish"; shift ;;
    --check-only) MODE="check";   shift ;;
    --resume)     MODE="publish"; RESUME_FROM="${2:-}"; shift 2 ;;
    -h|--help)    sed -n '2,22p' "$0"; exit 0 ;;
    *) echo "unknown flag: $1" >&2; exit 2 ;;
  esac
done

cd "$(dirname "$0")/.."

say()  { printf '\n\033[1;34m== %s ==\033[0m\n' "$*"; }
ok()   { printf '\033[32m  ✓ %s\033[0m\n' "$*"; }
die()  { printf '\033[31m  ✗ %s\033[0m\n' "$*" >&2; exit 1; }

# ---------------------------------------------------------------- pre-flight
say "Pre-flight"
command -v cargo >/dev/null || die "cargo not found in PATH"
ok "cargo: $(cargo --version)"

# All publishable crates share one version; read it from the first for the tag.
VERSION=$(grep -E '^version\s*=' "${PUBLISH[0]}/Cargo.toml" | head -1 | sed -E 's/.*"([^"]+)".*/\1/')
[[ -n "$VERSION" ]] || die "could not read version from ${PUBLISH[0]}/Cargo.toml"
ok "version: $VERSION"

if [[ "$MODE" == "publish" ]]; then
  git diff --quiet && git diff --cached --quiet || die "working tree not clean — commit or stash first"
  ok "working tree clean (branch: $(git rev-parse --abbrev-ref HEAD))"
fi

# ---------------------------------------------------------------- validate
say "cargo check --workspace"
cargo check --workspace
ok "workspace builds"

if [[ "$MODE" == "check" ]]; then
  say "--check-only: done"
  exit 0
fi

# ---------------------------------------------------------------- publish
# Build the package set, dropping anything before --resume <crate>.
PKG_ARGS=()
skipping=false
[[ -n "$RESUME_FROM" ]] && skipping=true
for crate in "${PUBLISH[@]}"; do
  if $skipping; then
    [[ "$crate" == "$RESUME_FROM" ]] && skipping=false || { echo "  (skip $crate)"; continue; }
  fi
  PKG_ARGS+=(-p "$crate")
done
[[ ${#PKG_ARGS[@]} -gt 0 ]] || die "no crates selected (unknown --resume target '$RESUME_FROM'?)"

if [[ "$MODE" == "dry-run" ]]; then
  say "Dry-run $VERSION — nothing is pushed to crates.io"
  cargo publish --dry-run --allow-dirty "${PKG_ARGS[@]}"
  ok "dry-run OK"
  exit 0
fi

say "Publishing $VERSION to crates.io"
cargo publish "${PKG_ARGS[@]}"
ok "published $VERSION"

# ---------------------------------------------------------------- tag
TAG="v$VERSION"
say "Tagging $TAG"
if git rev-parse -q --verify "refs/tags/$TAG" >/dev/null; then
  ok "tag $TAG already exists — skipping"
else
  git tag -a "$TAG" -m "Release $TAG across all crates"
  git push origin "$TAG"
  ok "tagged and pushed $TAG"
fi

say "Done"
