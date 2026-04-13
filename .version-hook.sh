#!/bin/bash
if [ -z "$1" ]; then
  echo "Error: Version number required"
  exit 1
fi
NEW_VERSION="$1"
sed -i.bak "s/^version = \".*\"/version = \"$NEW_VERSION\"/" Cargo.toml && rm Cargo.toml.bak
echo "Updated conforme to version $NEW_VERSION"
