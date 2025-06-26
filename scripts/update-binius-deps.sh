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
if [ "$LATEST_COMMIT" = "$CURRENT_COMMIT" ] && [ -n "$CURRENT_COMMIT" ]; then
    echo "✅ Dependencies are already up to date!"
    exit 0
fi

echo "🔄 Updating Binius dependencies..."

# Update all Binius dependencies to the latest commit
if [ -z "$CURRENT_COMMIT" ]; then
    # If current commit is empty, replace empty rev fields
    sed -i.backup 's/rev = ""/rev = "'$LATEST_COMMIT'"/' Cargo.toml
else
    # If current commit exists, replace it
    sed -i.backup "s/$CURRENT_COMMIT/$LATEST_COMMIT/g" Cargo.toml
fi

echo "📦 Updating Cargo.lock..."
cargo update

echo "🔍 Running basic checks..."
if ! cargo check --workspace; then
    echo "❌ Cargo check failed! Please fix build issues before proceeding."
    echo ""
    exit 1
fi

echo "✅ Binius dependencies updated successfully!"
echo "Updated from: $CURRENT_COMMIT"
echo "Updated to:   $LATEST_COMMIT"
echo ""
echo "🚀 Ready to commit and push your changes!" 
