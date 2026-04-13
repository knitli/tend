#!/usr/bin/env bash
#
# Bump version across all manifests, generate changelog, commit, and tag.
#
# Usage:
#   ./scripts/release.sh 0.2.0
#   ./scripts/release.sh patch   # 0.1.0 → 0.1.1
#   ./scripts/release.sh minor   # 0.1.0 → 0.2.0
#   ./scripts/release.sh major   # 0.1.0 → 1.0.0
#
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

# ---------------------------------------------------------------------------
# Resolve the new version
# ---------------------------------------------------------------------------
current_version() {
  grep -m1 '"version"' src-tauri/tauri.conf.json \
    | sed 's/.*"version": *"\([^"]*\)".*/\1/'
}

CURRENT=$(current_version)

case "${1:-}" in
  major)
    IFS='.' read -r maj min pat <<< "$CURRENT"
    NEW="$((maj + 1)).0.0"
    ;;
  minor)
    IFS='.' read -r maj min pat <<< "$CURRENT"
    NEW="${maj}.$((min + 1)).0"
    ;;
  patch)
    IFS='.' read -r maj min pat <<< "$CURRENT"
    NEW="${maj}.${min}.$((pat + 1))"
    ;;
  [0-9]*)
    NEW="$1"
    ;;
  *)
    echo "Usage: $0 <major|minor|patch|X.Y.Z>"
    exit 1
    ;;
esac

echo "Bumping $CURRENT → $NEW"

# ---------------------------------------------------------------------------
# Update version in all three manifests
# ---------------------------------------------------------------------------

# Cargo.toml (workspace)
sed -i "s/^version = \"$CURRENT\"/version = \"$NEW\"/" Cargo.toml

# src-tauri/tauri.conf.json
sed -i "s/\"version\": \"$CURRENT\"/\"version\": \"$NEW\"/" src-tauri/tauri.conf.json

# package.json
sed -i "s/\"version\": \"$CURRENT\"/\"version\": \"$NEW\"/" package.json

# ---------------------------------------------------------------------------
# Regenerate Cargo.lock to pick up workspace version change
# ---------------------------------------------------------------------------
cargo generate-lockfile --quiet

# ---------------------------------------------------------------------------
# Generate / update changelog
# ---------------------------------------------------------------------------
if command -v git-cliff &>/dev/null; then
  git-cliff --tag "v${NEW}" --output CHANGELOG.md
  echo "CHANGELOG.md updated"
else
  echo "Warning: git-cliff not found — skipping changelog generation"
fi

# ---------------------------------------------------------------------------
# Commit and tag
# ---------------------------------------------------------------------------
git add Cargo.toml Cargo.lock src-tauri/tauri.conf.json package.json CHANGELOG.md
git commit -m "chore: release v${NEW}"
git tag -a "v${NEW}" -m "v${NEW}"

echo ""
echo "Done. To publish:"
echo "  git push origin main --follow-tags"
