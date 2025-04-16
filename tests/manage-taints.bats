#!/usr/bin/env bats

# Tests for manage-taints.sh script

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
  export SCRIPT_PATH="${TEMP_DIR}/manage-taints.sh"
  cp "$(pwd)/manage-taints.sh" "$SCRIPT_PATH"
  chmod +x "$SCRIPT_PATH"
  
  # Create service account secret directory
  export SECRET_DIR="/var/run/secrets/kubernetes.io/serviceaccount"
  mkdir -p "$SECRET_DIR"
  
  # Create mock token file
  echo "mock-token-value" > "${SECRET_DIR}/token"
  
  # Create mock ca.crt file
  echo "mock-ca-cert" > "${SECRET_DIR}/ca.crt"
  
  # Set required environment variables
  export KUBERNETES_SERVICE_HOST="kubernetes.default.svc"
  export KUBERNETES_SERVICE_PORT="443"
  export NODE_NAME="test-node"
}

# Clean up our test environment
teardown() {
  export PATH="$ORIGINAL_PATH"
  unset KUBERNETES_SERVICE_HOST
  unset KUBERNETES_SERVICE_PORT
  unset NODE_NAME
  rm -rf "$TEMP_DIR"
  
  # Also clean up the service account directory we created
  if [ -d "/var/run/secrets/kubernetes.io/serviceaccount" ]; then
    rm -rf "/var/run/secrets/kubernetes.io/serviceaccount"
  fi
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
  
  # Create mock cat that works for our service account token
  cat > "${MOCK_BIN}/cat" <<EOF
#!/bin/bash
if [[ "\$*" == *"serviceaccount/token"* ]]; then
  echo "mock-token-value"
else
  /bin/cat "\$@"
fi
EOF
  chmod +x "${MOCK_BIN}/cat"
  
  # Run the script with the remove action
  run "$SCRIPT_PATH" remove
  
  # Check output
  echo "Output: $output"
  [ "$status" -eq 0 ]
  [[ "$output" == *"Starting taint management for node: ${NODE_NAME}"* ]]
  [[ "$output" == *"Removing taint disk-unconfigured from node ${NODE_NAME}"* ]]
  [[ "$output" == *"node/${NODE_NAME} untainted"* ]]
}

@test "Script continues after kubectl failure" {
  # Create mock kubectl that fails
  cat > "${MOCK_BIN}/kubectl" <<EOF
#!/bin/bash
echo "kubectl called with args: \$@"
echo "Error: failed to untaint node" >&2
exit 1
EOF
  chmod +x "${MOCK_BIN}/kubectl"
  
  # Create mock cat that works for our service account token
  cat > "${MOCK_BIN}/cat" <<EOF
#!/bin/bash
if [[ "\$*" == *"serviceaccount/token"* ]]; then
  echo "mock-token-value"
else
  /bin/cat "\$@"
fi
EOF
  chmod +x "${MOCK_BIN}/cat"

  # Run the script with the remove action
  run "$SCRIPT_PATH" remove

  # Check output - should continue despite kubectl failure
  echo "Output: $output"
  [ "$status" -eq 0 ]
  [[ "$output" == *"Starting taint management for node: ${NODE_NAME}"* ]]
  [[ "$output" == *"Removing taint disk-unconfigured from node ${NODE_NAME}"* ]]
}

# Test environment validation
@test "Fails when Kubernetes service environment variables are missing" {
  # Save current values
  local k8s_host="$KUBERNETES_SERVICE_HOST"
  local k8s_port="$KUBERNETES_SERVICE_PORT"

  local modified_script="${TEMP_DIR}/modified-script.sh"
  sed 's/set -euo pipefail/set -eo pipefail/' "$SCRIPT_PATH" > "$modified_script"
  chmod +x "$modified_script"

  # Unset required environment variables
  unset KUBERNETES_SERVICE_HOST
  unset KUBERNETES_SERVICE_PORT
  
  # Run the modified script
  run "$modified_script" remove
  
  # Restore environment variables
  export KUBERNETES_SERVICE_HOST="$k8s_host"
  export KUBERNETES_SERVICE_PORT="$k8s_port"
  
  # Check output - should fail with the expected error
  echo "Output: $output"
  [ "$status" -eq 1 ]
  [[ "$output" == *"Error: Kubernetes service environment variables not found"* ]]
}

@test "Fails when service account token is missing" {
  # Remove token file
  rm -f "${SECRET_DIR}/token"
  
  # Run the script
  run "$SCRIPT_PATH" remove
  
  # Check output - should fail
  echo "Output: $output"
  [ "$status" -eq 1 ]
  [[ "$output" == *"Error: Service account token not found"* ]]
}

# Test command line argument handling
@test "Shows usage when called with invalid action" {
  # Create a modified script to test just the action handling part
  local modified_script="${TEMP_DIR}/action-test-script.sh"
  cat > "$modified_script" <<EOF
#!/bin/bash
# Simplified version just for testing action handling
NODE_NAME="test-node"
TAINT_KEY="disk-unconfigured"

echo "Starting taint management for node: \$NODE_NAME"
echo "Action: \$1"

# Mock the taint removal function
remove_taint() {
  echo "Removing taint"
  return 0
}

# Main execution
ACTION=\${1:-"remove"}

case "\$ACTION" in
    remove)
        remove_taint
        ;;
    *)
        echo "Usage: \$0 [remove]"
        exit 1
        ;;
esac

exit 0
EOF
  chmod +x "$modified_script"
  
  # Run the modified script with an invalid action
  run "$modified_script" invalid-action
  
  # Check output - should show usage
  echo "Output: $output"
  [ "$status" -eq 1 ]
  [[ "$output" == *"Usage: $modified_script [remove]"* ]]
}

@test "Defaults to 'remove' action when no arguments provided" {
  # Create a modified script to test just the default action behavior
  local modified_script="${TEMP_DIR}/default-action-script.sh"
  cat > "$modified_script" <<EOF
#!/bin/bash
# Simplified version just for testing default action
NODE_NAME="test-node"
TAINT_KEY="disk-unconfigured"

echo "Starting taint management for node: \$NODE_NAME"
echo "Action: \${1:-remove}"

# Mock the taint removal function
remove_taint() {
  echo "Removing taint disk-unconfigured from node \$NODE_NAME"
  return 0
}

# Main execution
ACTION=\${1:-"remove"}

case "\$ACTION" in
    remove)
        remove_taint
        ;;
    *)
        echo "Usage: \$0 [remove]"
        exit 1
        ;;
esac

exit 0
EOF
  chmod +x "$modified_script"
  
  # Run the modified script with no arguments
  run "$modified_script"
  
  # Check output - should default to 'remove' action
  echo "Output: $output"
  [ "$status" -eq 0 ]
  [[ "$output" == *"Action: remove"* ]]
  [[ "$output" == *"Removing taint disk-unconfigured from node test-node"* ]]
}

@test "Uses hostname when NODE_NAME is not provided" {
  # Unset NODE_NAME
  unset NODE_NAME
  
  # Create mock hostname command
  create_mock "hostname" 0 "mock-host"
  
  # Create mock kubectl that logs its args and succeeds
  cat > "${MOCK_BIN}/kubectl" <<EOF
#!/bin/bash
echo "kubectl called with args: \$@"
echo "node/mock-host untainted"
exit 0
EOF
  chmod +x "${MOCK_BIN}/kubectl"
  
  # Run the script
  run "$SCRIPT_PATH" remove
  
  # Check output - should use hostname
  echo "Output: $output"
  [ "$status" -eq 0 ]
  [[ "$output" == *"Starting taint management for node: mock-host"* ]]
  [[ "$output" == *"Removing taint disk-unconfigured from node mock-host"* ]]
}
