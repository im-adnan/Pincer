#!/bin/bash

# Exit on any error
set -e

# Change to the project root directory
cd "$(dirname "$0")/.."

if [ -z "$1" ]; then
    echo "Error: No version specified."
    echo "Usage: ./scripts/release.sh <new_version>"
    echo "Example: ./scripts/release.sh 1.5.0"
    exit 1
fi

# Extract the version without the 'v' prefix if the user included it
# (e.g., v1.5.0 -> 1.5.0)
RAW_VERSION=${1#v}
NEW_VERSION=$RAW_VERSION
CURRENT_BRANCH=$(git rev-parse --abbrev-ref HEAD)

echo "🚀 Starting release process for version $NEW_VERSION on branch $CURRENT_BRANCH..."

# 1. Run Unified Test Suite
echo "🧪 Running full test suite (Formatting, Linting, Rust Tests, Python Integration Tests)..."
python3 scripts/run_tests.py --ci

# 2. Run Release Build
echo "🔨 Building the release version..."
cargo build --release

# 3. Update Cargo.toml version
echo "📝 Updating version in Cargo.toml to $NEW_VERSION..."
# Works on macOS (BSD sed)
sed -i '' "s/^version = \".*\"/version = \"$NEW_VERSION\"/" Cargo.toml

# Verify that it updated
if ! grep -q "version = \"$NEW_VERSION\"" Cargo.toml; then
    echo "❌ Failed to update version in Cargo.toml"
    exit 1
fi

# 4. Git Commit and Tag
echo "📦 Committing and tagging..."
git add Cargo.toml Cargo.lock || true
git commit -m "chore: release version $NEW_VERSION"
git tag -a "v$NEW_VERSION" -m "Release v$NEW_VERSION"

echo "✅ Release $NEW_VERSION complete!"
echo "☁️ To complete the release, push the commit and tag to GitHub:"
echo "  git push origin HEAD"
echo "  git push origin \"v$NEW_VERSION\""
echo ""
echo "✅ Success! Once pushed, GitHub Actions will automatically build and release v$NEW_VERSION."
