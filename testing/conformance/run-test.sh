#!/bin/bash
set -euo pipefail

BINARY=$(realpath "$1")
CASE_DIR="$2"
SUITE="$3"

WORKDIR=$(mktemp -d)
ACTUAL_STDOUT=$(mktemp)
ACTUAL_STDERR=$(mktemp)
trap 'rm -rf "$WORKDIR" "$ACTUAL_STDOUT" "$ACTUAL_STDERR"' EXIT

if [ -d "$CASE_DIR/input" ]; then
    cp -a "$CASE_DIR/input/." "$WORKDIR/"
fi

read -r -a COMMAND < "$CASE_DIR/command"

set +e
(cd "$WORKDIR" && "$BINARY" "${COMMAND[@]}" >"$ACTUAL_STDOUT" 2>"$ACTUAL_STDERR")
STATUS=$?
set -e

EXPECTED_STATUS=$(cat "$CASE_DIR/status")
if [ "$STATUS" -ne "$EXPECTED_STATUS" ]; then
    echo "FAIL: $SUITE/$(basename "$CASE_DIR") status"
    echo "--- Expected status ---"
    echo "$EXPECTED_STATUS"
    echo "--- Actual status ---"
    echo "$STATUS"
    exit 1
fi

if ! diff -u "$CASE_DIR/stdout" "$ACTUAL_STDOUT"; then
    echo "FAIL: $SUITE/$(basename "$CASE_DIR") stdout"
    exit 1
fi

if ! diff -u "$CASE_DIR/stderr" "$ACTUAL_STDERR"; then
    echo "FAIL: $SUITE/$(basename "$CASE_DIR") stderr"
    exit 1
fi

if ! diff -ru "$CASE_DIR/expected" "$WORKDIR"; then
    echo "FAIL: $SUITE/$(basename "$CASE_DIR") workspace"
    exit 1
fi

echo "PASS: $SUITE/$(basename "$CASE_DIR")"
