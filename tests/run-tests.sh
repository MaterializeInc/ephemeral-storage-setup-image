#!/usr/bin/env bash

# Script to run all tests

set -eo pipefail

# Directory where tests are located
TEST_DIR="$(dirname "$(readlink -f "$0")")"

echo "=== Running tests for detect-disks.sh ==="
"${TEST_DIR}/test-detect-disks.sh"

echo ""
echo "=== Running tests for configure-lvm.sh ==="
"${TEST_DIR}/test-configure-lvm.sh"

echo ""
echo "=== Running tests for configure-swap.sh ==="
"${TEST_DIR}/test-configure-swap.sh"

echo ""
echo "=== Running tests for remove-taint.sh ==="
"${TEST_DIR}/test-remove-taint.sh"

echo ""
echo "All tests completed!"
