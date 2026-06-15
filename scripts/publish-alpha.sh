#!/usr/bin/env bash
# Publish all 11 publishable crates to crates.io at the version pinned in
# RELEASE_VERSION below.
#
# Versions and repository URLs are already set in each Cargo.toml — this script
# only validates and publishes; it does NOT mutate Cargo.toml. To re-publish
# bump RELEASE_VERSION here AND every Cargo.toml first.
#
# Modes:
#   bash scripts/publish-alpha.sh                   # dry-run: cargo publish --dry-run
#   bash scripts/publish-alpha.sh --publish         # really publish to crates.io
#   bash scripts/publish-alpha.sh --check-only      # only run `cargo check --workspace`
#   bash scripts/publish-alpha.sh --resume <crate>  # restart publish from <crate>
#
# Prerequisites:
#   - cargo present
#   - Run from a Wolfram-equipped machine — the -sys crates need Wolfram C
#     headers (discovered via wolfram-app-discovery) at `cargo package` time.
#   - `cargo login` done with a crates.io token that has publish rights on every
#     crate listed in ORDER. Your manager must add you as an owner first:
#       cargo owner --add riccardodivirgilio <crate>
#   - For --publish: clean working tree (script bails otherwise).

set -euo pipefail

# The single version every publishable crate is pinned to in Cargo.toml.
# Bump here AND in every Cargo.toml if you need to re-publish — versions on
# crates.io cannot be reused once published, only yanked.
#
# Alpha versions are NOT picked up by normal `^x.y.z` requirements, so existing
# downstream users won't auto-resolve to this release. Internal deps inside the
# workspace use `=0.6.0-alpha.1` exact-pin (required for alpha pre-releases).
RELEASE_VERSION="0.6.0-alpha.1"

# Publish order = topological by dep graph (leaves first).
# 11 publishable crates after the wxf/export refactor on feature/wxf.
ORDER=(
  wolfram-app-discovery         # no internal deps
  wolfram-wxf-macros            # no internal deps (proc-macro)
  wolfram-export-macros         # no internal deps (proc-macro)
  wolfram-wxf                   # deps: wxf-macros
  wolfram-expr                  # deps: wxf
  wolfram-export-core           # deps: expr, wxf
  wstp-sys                      # build-dep: app-discovery
  wolfram-library-link-sys      # build-dep: app-discovery, expr
  wstp                          # deps: expr, wxf (dev-dep: app-discovery)
  wolfram-library-link          # deps: export-core, export-macros, expr, lib-link-sys, wstp (optional)
  wolfram-export                # deps: export-core, export-macros, expr, lib-link, lib-link-sys, wxf, wstp
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

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

say()  { printf '\n\033[1;34m== %s ==\033[0m\n' "$*"; }
ok()   { printf '\033[32m  ✓ %s\033[0m\n' "$*"; }
warn() { printf '\033[33m  ! %s\033[0m\n' "$*"; }
die()  { printf '\033[31m  ✗ %s\033[0m\n' "$*" >&2; exit 1; }

# ---------------------------------------------------------------- pre-flight
say "Pre-flight checks"
command -v cargo >/dev/null || die "cargo not found in PATH"
ok "cargo present: $(cargo --version)"
ok "release version: $RELEASE_VERSION"

# Sanity-check that every crate in ORDER actually has the expected version in
# its Cargo.toml — protects against publishing a stale tree.
for crate in "${ORDER[@]}"; do
  actual=$(grep -E '^version\s*=' "$crate/Cargo.toml" | head -1 | sed -E 's/.*"([^"]+)".*/\1/')
  if [[ "$actual" != "$RELEASE_VERSION" ]]; then
    die "$crate/Cargo.toml version is '$actual', expected '$RELEASE_VERSION'"
  fi
done
ok "all 11 crates pinned to $RELEASE_VERSION"

# Confirm repository URL points at the monorepo for every publishable crate.
for crate in "${ORDER[@]}"; do
  if ! grep -q 'WolframResearch/wolfram-rust-library' "$crate/Cargo.toml"; then
    die "$crate/Cargo.toml repository URL is not WolframResearch/wolfram-rust-library"
  fi
done
ok "all 11 crates point repository → WolframResearch/wolfram-rust-library"

if [[ "$MODE" == "publish" ]]; then
  if ! git diff --quiet || ! git diff --cached --quiet; then
    die "working tree not clean — commit or stash first"
  fi
  ok "working tree clean (branch: $(git rev-parse --abbrev-ref HEAD))"
fi

# ---------------------------------------------------------------- validate
say "Validating with cargo check --workspace"
cargo check --workspace
ok "workspace builds at $RELEASE_VERSION"

if [[ "$MODE" == "check" ]]; then
  say "--check-only: stopping here"
  exit 0
fi

# ---------------------------------------------------------------- publish
PUBLISH_FLAGS=()
if [[ "$MODE" == "dry-run" ]]; then
  PUBLISH_FLAGS+=(--dry-run --allow-dirty)
  say "DRY RUN — nothing will be pushed to crates.io"
else
  say "REAL PUBLISH — pushing to crates.io"
fi

skipping=true
[[ -z "$RESUME_FROM" ]] && skipping=false

for crate in "${ORDER[@]}"; do
  if $skipping; then
    if [[ "$crate" == "$RESUME_FROM" ]]; then
      skipping=false
    else
      echo "  (skip $crate — resuming from $RESUME_FROM)"
      continue
    fi
  fi

  say "Publishing $crate $RELEASE_VERSION"
  (
    cd "$crate"
    cargo publish ${PUBLISH_FLAGS[@]+"${PUBLISH_FLAGS[@]}"}
  )
  if [[ "$MODE" == "publish" ]]; then
    ok "$crate $RELEASE_VERSION published"
    # Cargo ≥1.66 waits for the crate to land in the sparse index before
    # returning, so the next crate sees this one. If you hit "not found"
    # errors on older cargo versions, uncomment:
    # sleep 20
  else
    ok "$crate $RELEASE_VERSION dry-run OK"
  fi
done

say "Done"
if [[ "$MODE" == "publish" ]]; then
  echo "Suggested next step:"
  echo "  git tag -a v$RELEASE_VERSION -m 'Release v$RELEASE_VERSION across all crates'"
  echo "  git push origin v$RELEASE_VERSION"
fi
