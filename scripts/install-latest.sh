#!/usr/bin/env bash
# Download and install the latest (or a specific) tend .deb release.
#
# Usage:
#   scripts/install-latest.sh            # latest release
#   scripts/install-latest.sh v0.1.8     # specific tag
#   scripts/install-latest.sh 0.1.8      # bare version also accepted
#
# Requires: gh (authenticated), dpkg, sudo.

set -euo pipefail

REPO="knitli/tend"
TMPDIR="$(mktemp -d -t tend-install-XXXXXX)"
trap 'rm -rf "$TMPDIR"' EXIT

if [[ $# -gt 0 ]]; then
  TAG="$1"
  [[ "$TAG" != v* ]] && TAG="v$TAG"
else
  TAG="$(gh release view --repo "$REPO" --json tagName -q .tagName)"
fi

echo "→ fetching $REPO $TAG .deb into $TMPDIR"
gh release download "$TAG" \
  --repo "$REPO" \
  --pattern '*_amd64.deb' \
  --dir "$TMPDIR"

DEB="$(find "$TMPDIR" -maxdepth 1 -name '*.deb' | head -n1)"
if [[ -z "$DEB" ]]; then
  echo "error: no .deb found in release $TAG" >&2
  exit 1
fi

echo "→ installing $(basename "$DEB")"
sudo dpkg -i "$DEB"

echo "✓ installed $(dpkg -l tend | awk '/^ii/ {print $2, $3}')"
