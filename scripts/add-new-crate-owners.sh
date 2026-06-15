#!/usr/bin/env bash
# Add jfultz and arnoudbuzing as owners on the 5 new crates introduced in
# 0.6.0-alpha.2 (serialize + export refactor). Idempotent — cargo owner --add is
# a no-op if the user is already an owner (it just sends another invite).
#
# Prerequisites:
#   - These 5 crates already published to crates.io (so they exist as owned crates)
#   - `cargo login` done as the current sole owner (the account that ran the first
#     `cargo publish` is the sole initial owner; only owners can grant ownership)
#
# Usage:
#   bash scripts/add-new-crate-owners.sh                 # invite both users on all 5 new crates
#   bash scripts/add-new-crate-owners.sh --all           # also include the 6 existing crates
#   bash scripts/add-new-crate-owners.sh --user U        # only invite one user (e.g. just jfultz)
#   bash scripts/add-new-crate-owners.sh --list-only     # just print current owners, no changes

set -euo pipefail

NEW_CRATES=(
  wolfram-serialize
  wolfram-serialize-macros
  wolfram-export
  wolfram-export-core
  wolfram-export-macros
)

EXISTING_CRATES=(
  wolfram-app-discovery
  wolfram-expr
  wolfram-library-link
  wolfram-library-link-sys
  wstp
  wstp-sys
)

USERS=(jfultz arnoudbuzing)
SCOPE="new"
LIST_ONLY=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --all)        SCOPE="all"; shift ;;
    --new)        SCOPE="new"; shift ;;
    --user)       USERS=("$2"); shift 2 ;;
    --list-only)  LIST_ONLY=1; shift ;;
    -h|--help)    sed -n '2,15p' "$0"; exit 0 ;;
    *) echo "unknown flag: $1" >&2; exit 2 ;;
  esac
done

CRATES=("${NEW_CRATES[@]}")
[[ "$SCOPE" == "all" ]] && CRATES+=("${EXISTING_CRATES[@]}")

say()  { printf '\n\033[1;34m== %s ==\033[0m\n' "$*"; }
ok()   { printf '\033[32m  ✓ %s\033[0m\n' "$*"; }
warn() { printf '\033[33m  ! %s\033[0m\n' "$*"; }
die()  { printf '\033[31m  ✗ %s\033[0m\n' "$*" >&2; exit 1; }

command -v cargo >/dev/null || die "cargo not found in PATH"

#-----------------------------------------------------------------
# List-only mode
#-----------------------------------------------------------------
if [[ "$LIST_ONLY" == "1" ]]; then
  say "Current owners (read-only)"
  for crate in "${CRATES[@]}"; do
    echo "-- $crate --"
    cargo owner --list "$crate" 2>&1 | sed 's/^/    /'
  done
  exit 0
fi

#-----------------------------------------------------------------
# Add owners
#-----------------------------------------------------------------
say "Adding owners to ${#CRATES[@]} crate(s): ${CRATES[*]}"
echo "Users: ${USERS[*]}"

failed=()
for crate in "${CRATES[@]}"; do
  echo
  echo "-- $crate --"
  for user in "${USERS[@]}"; do
    printf "  cargo owner --add %s %s ... " "$user" "$crate"
    if out=$(cargo owner --add "$user" "$crate" 2>&1); then
      echo "ok"
    else
      # crates.io returns a clear error if the user is already an owner — surface
      # the message but don't fail the whole script
      if echo "$out" | grep -qiE 'already (an? )?owner|invite has already been sent'; then
        echo "already an owner / invite already pending"
      else
        echo "FAILED"
        echo "$out" | sed 's/^/      /'
        failed+=("$crate:$user")
      fi
    fi
  done
done

echo
if [[ ${#failed[@]} -eq 0 ]]; then
  ok "All ownership invites sent or already present."
else
  warn "Some failed:"
  printf '    %s\n' "${failed[@]}"
  exit 1
fi

echo
echo "Each invited user must accept the invitation at:"
echo "  https://crates.io/me/pending-invites"
