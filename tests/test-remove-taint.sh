#!/usr/bin/env bash

# Tests for remove-taint.sh script

set -euo pipefail

source "$(dirname "$0")/test_helpers.sh"


test_successfully_removes_taint() {
  # Create mock kubectl that logs its args and succeeds
  cat > "${MOCK_BIN}/kubectl" <<EOF
#!/bin/bash
if [[ "\$*" == *"taint nodes ${NODE_NAME} disk-unconfigured-"* ]]; then
  echo "node/${NODE_NAME} untainted"
else
  echo "Unexpected kubectl command: \$@"
  exit 1
fi
exit 0
EOF
  chmod +x "${MOCK_BIN}/kubectl"

  local output="$("$REMOVE_TAINT_SCRIPT_PATH")"
  assert_output_contains "${output}" "Removing taint disk-unconfigured from node $NODE_NAME"
  assert_output_contains "${output}" "node/${NODE_NAME} untainted"
}

test_handles_taint_already_removed() {
  # Create mock kubectl that fails on first call but succeeds on second
  cat > "${MOCK_BIN}/kubectl" <<EOF
#!/bin/bash
if [[ "\$*" == *"taint nodes"* ]]; then
  echo "Error: failed to untaint node" >&2
  exit 1
elif [[ "\$*" == *"get node"* ]]; then
  # Return empty string to indicate taint is not present
  echo ""
  exit 0
else
  echo "Unexpected kubectl command: \$@"
  exit 1
fi
EOF
  chmod +x "${MOCK_BIN}/kubectl"

  local output="$("$REMOVE_TAINT_SCRIPT_PATH" 2>/dev/null)"
  assert_output_contains "${output}" "Removing taint disk-unconfigured from node $NODE_NAME"
  assert_output_contains "${output}" "Note: Taint was already removed from the node"
}

test_fails_when_taint_still_there() {
  # Create mock kubectl that fails on first call and finds taint still exists
  cat > "${MOCK_BIN}/kubectl" <<EOF
#!/bin/bash
if [[ "\$*" == *"taint nodes"* ]]; then
  echo "Error: failed to untaint node" >&2
  exit 1
elif [[ "\$*" == *"get node"* ]]; then
  # Return disk-unconfigured to indicate taint still exists
  echo "disk-unconfigured"
  exit 0
else
  echo "Unexpected kubectl command: \$@"
  exit 1
fi
EOF
  chmod +x "${MOCK_BIN}/kubectl"

  local output="$(run_expecting_rc 1 "$REMOVE_TAINT_SCRIPT_PATH" 2>/dev/null)"

  assert_output_contains "${output}" "Error: Failed to remove taint and it still exists on the node"
}

test_fails_when_node_name_not_set() {
  unset NODE_NAME

  local output="$(run_expecting_rc 1 "$REMOVE_TAINT_SCRIPT_PATH" 2>/dev/null)"

  assert_output_contains "${output}" "Error: NODE_NAME environment variable is required but not set"
}


run_test test_successfully_removes_taint
run_test test_handles_taint_already_removed
run_test test_fails_when_taint_still_there
run_test test_fails_when_node_name_not_set
