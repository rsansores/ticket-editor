#!/usr/bin/env bash
# Set the release version in lockstep across the Rust workspace and the npm
# package. The wasm the editor ships is compiled from ticket-core, so the crate
# and the npm package MUST always carry the same version or the 1:1 preview/print
# parity guarantee silently breaks.
#
# Usage: scripts/set-version.sh 1.2.3
set -euo pipefail

VERSION="${1:?usage: set-version.sh X.Y.Z}"
if ! printf '%s' "$VERSION" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.]+)?(\+[0-9A-Za-z.]+)?$'; then
  echo "error: '$VERSION' is not a valid semver version" >&2
  exit 1
fi

cd "$(dirname "$0")/.."

# Rust workspace version (single source of truth) — the only line-anchored
# `version = "..."` in the root manifest is [workspace.package].
sed -i -E "s/^version = \"[^\"]+\"/version = \"$VERSION\"/" Cargo.toml

# npm package version (edit via node to preserve JSON structure).
node -e '
  const fs = require("fs");
  const f = "packages/ticket-editor/package.json";
  const pkg = JSON.parse(fs.readFileSync(f, "utf8"));
  pkg.version = process.argv[1];
  fs.writeFileSync(f, JSON.stringify(pkg, null, 2) + "\n");
' "$VERSION"

echo "set version $VERSION in Cargo.toml and packages/ticket-editor/package.json"
