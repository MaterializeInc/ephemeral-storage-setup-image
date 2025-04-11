#!/usr/bin/env bash

# Script to run all tests

set -eo pipefail

# Check if bats is installed
if ! command -v bats &> /dev/null; then
    echo "Error: bats is not installed. Please install it to run the tests."
    echo "Instructions: https://bats-core.readthedocs.io/en/stable/installation.html"
    exit 1
fi

# Directory where tests are located
TEST_DIR="$(dirname "$(readlink -f "$0")")"

echo "=== Running tests for configure-disks.sh ==="
bats "${TEST_DIR}/configure-disks.bats"

echo ""
echo "=== Running tests for manage-taints.sh ==="
bats "${TEST_DIR}/manage-taints.bats"

echo ""
echo "All tests completed!"
