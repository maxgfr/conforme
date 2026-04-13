#!/bin/bash
if [ -z "$1" ]; then
  echo "Error: Version number required"
  exit 1
fi
NEW_VERSION="$1"
sed -i.bak "s/^version = \".*\"/version = \"$NEW_VERSION\"/" Cargo.toml && rm Cargo.toml.bak
echo "Updated Cargo.toml to version $NEW_VERSION"
sed -i.bak '/^name = "conforme"$/{n;s/^version = ".*"$/version = "'"$NEW_VERSION"'"/;}' Cargo.lock && rm Cargo.lock.bak
echo "Updated Cargo.lock to version $NEW_VERSION"
