#!/usr/bin/env bats

# Tests for remove-taint.sh script

# Create a temporary directory for our test environment
setup() {
  # Create temp directory for test artifacts
  TEMP_DIR="$(mktemp -d)"
  export TEMP_DIR

  # Save original path
  export ORIGINAL_PATH="$PATH"
  
  # Create mock bin directory for our mock commands
  export MOCK_BIN="${TEMP_DIR}/bin"
  mkdir -p "$MOCK_BIN"
  
  # Add our mock bin to the front of the PATH
  export PATH="$MOCK_BIN:$PATH"
  
  # Copy the script for testing
  export SCRIPT_PATH="${TEMP_DIR}/remove-taint.sh"
  cp "$(pwd)/remove-taint.sh" "$SCRIPT_PATH"
  chmod +x "$SCRIPT_PATH"
  
  # Set required environment variables
  export NODE_NAME="test-node"
}

# Clean up our test environment
teardown() {
  export PATH="$ORIGINAL_PATH"
  unset NODE_NAME
  rm -rf "$TEMP_DIR"
}

# Helper to create mock commands
create_mock() {
  local cmd="$1"
  local exit_code="${2:-0}"
  local output="$3"
  
  cat > "${MOCK_BIN}/${cmd}" <<EOF
#!/bin/bash
echo "${output}"
exit ${exit_code}
EOF
  chmod +x "${MOCK_BIN}/${cmd}"
}

# Test basic functionality
@test "Script successfully removes taint" {
  # Create mock kubectl that logs its args and succeeds
  cat > "${MOCK_BIN}/kubectl" <<EOF
#!/bin/bash
echo "kubectl called with args: \$@"
if [[ "\$*" == *"taint nodes ${NODE_NAME} disk-unconfigured-"* ]]; then
  echo "node/${NODE_NAME} untainted"
else
  echo "Unexpected kubectl command: \$@"
  exit 1
fi
exit 0
EOF
  chmod +x "${MOCK_BIN}/kubectl"
  
  # Run the script (no arguments needed now)
  run "$SCRIPT_PATH"
  
  # Check output
  echo "Output: $output"
  [ "$status" -eq 0 ]
  [[ "$output" == *"Removing taint disk-unconfigured from node $NODE_NAME"* ]]
  [[ "$output" == *"node/${NODE_NAME} untainted"* ]]
}

@test "Script checks if taint exists when kubectl fails initially" {
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

  # Run the script
  run "$SCRIPT_PATH"

  # Check output - should indicate taint was already removed
  echo "Output: $output"
  [ "$status" -eq 0 ]
  [[ "$output" == *"Starting taint removal for node: ${NODE_NAME}"* ]]
  [[ "$output" == *"Removing taint disk-unconfigured from node ${NODE_NAME}"* ]]
  [[ "$output" == *"Note: Taint was already removed from the node"* ]]
}

@test "Script fails when kubectl finds taint still exists after attempted removal" {
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

  # Run the script
  run "$SCRIPT_PATH"

  # Check output - should fail because taint still exists
  echo "Output: $output"
  [ "$status" -eq 1 ]
  [[ "$output" == *"Starting taint removal for node: ${NODE_NAME}"* ]]
  [[ "$output" == *"Error: Failed to remove taint and it still exists on the node"* ]]
}

@test "Fails with error when NODE_NAME is not provided" {
  # Unset NODE_NAME
  unset NODE_NAME

  # Run the script
  run "$SCRIPT_PATH"

  # Check output - should fail with appropriate error message
  echo "Output: $output"
  [ "$status" -eq 1 ]
  [[ "$output" == *"Error: NODE_NAME environment variable is required but not set"* ]]
}
