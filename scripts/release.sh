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

# 2. Run Release Build (local validation only)
echo "🔨 Building release locally to validate..."
cargo build --release

# 3. Create git tag (version is injected dynamically by CI from the tag name)
echo "🏷️ Creating git tag v$NEW_VERSION..."
git tag -a "v$NEW_VERSION" -m "Release v$NEW_VERSION"

echo "✅ Release $NEW_VERSION ready!"
echo "☁️ To complete the release, push the tag to GitHub:"
echo "  git push origin \"v$NEW_VERSION\""
echo ""
echo "✅ Once pushed, GitHub Actions will build, package and publish the release automatically."
