#!/usr/bin/env bash
# Regenerates the frontend TypeScript bindings from the Rust `domain` types.
#
# How it works:
# 1. `cargo test export_bindings` makes ts-rs write every #[ts(export)] type
#    into backend/bindings/ (ts-rs's default output folder).
# 2. We copy those files into frontend/src/api/generated/ (which is gitignored —
#    it is always regenerated, never hand-edited).
# 3. We write an index.ts that re-exports every generated file.
#
# Run from anywhere: ./scripts/typegen.sh
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BACKEND="$REPO_ROOT/backend"
OUT="$REPO_ROOT/frontend/src/api/generated"

cd "$BACKEND"
cargo test export_bindings >/dev/null

rm -rf "$OUT"
mkdir -p "$OUT"
cp -r bindings/* "$OUT/"

# Build index.ts: one "export * from './X';" line per generated .ts file.
rm -f "$OUT/index.ts"
for f in "$OUT"/*.ts; do
  name="$(basename "$f" .ts)"
  echo "export * from './$name';" >> "$OUT/index.ts"
done

echo "TypeScript bindings written to $OUT ($(ls "$OUT" | wc -l) files)"
