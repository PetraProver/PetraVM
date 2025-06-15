#!/bin/bash

# Script to manually update Binius dependencies to their latest commits
# Usage: ./scripts/update-binius-deps.sh

set -e

echo "🔍 Fetching latest Binius commit..."

# Get the latest commit from the Binius repository
LATEST_COMMIT=$(git ls-remote https://github.com/IrreducibleOSS/binius.git HEAD | cut -f1)
echo "Latest Binius commit: $LATEST_COMMIT"

# Get the current commit from Cargo.toml
CURRENT_COMMIT=$(grep 'binius_core.*rev.*=' Cargo.toml | sed 's/.*rev = "\([^"]*\)".*/\1/')
echo "Current Binius commit: $CURRENT_COMMIT"

# Check if update is needed
if [ "$LATEST_COMMIT" = "$CURRENT_COMMIT" ]; then
    echo "✅ Dependencies are already up to date!"
    exit 0
fi

echo "🔄 Updating Binius dependencies..."

# Update all Binius dependencies to the latest commit
sed -i.backup "s/$CURRENT_COMMIT/$LATEST_COMMIT/g" Cargo.toml

echo "📦 Updating Cargo.lock..."
cargo update

echo "🔍 Running basic checks..."
cargo check --workspace

echo "✅ Binius dependencies updated successfully!"
echo "Updated from: $CURRENT_COMMIT"
echo "Updated to:   $LATEST_COMMIT"
echo ""
echo "Dependencies updated:"
echo "- binius_core"
echo "- binius_fast_compute" 
echo "- binius_compute"
echo "- binius_field"
echo "- binius_hal"
echo "- binius_hash"
echo "- binius_m3"
echo "- binius_utils"
echo ""
echo "🚀 Ready to commit and push your changes!" 