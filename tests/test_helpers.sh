#!/usr/bin/env bash

# Helper functions for testing shell scripts

# Create a mock command with specified behavior
create_mock_command() {
  local cmd_path="$1"
  local response="$2"
  local exit_code="${3:-0}"
  
  mkdir -p "$(dirname "$cmd_path")"
  
  cat > "$cmd_path" <<EOF
#!/bin/bash
echo "$response"
exit $exit_code
EOF
  chmod +x "$cmd_path"
}

# Create a mock command that logs its arguments
create_logging_mock() {
  local cmd_path="$1"
  local response="$2"
  local exit_code="${3:-0}"
  local log_file="$4"
  
  mkdir -p "$(dirname "$cmd_path")"
  
  cat > "$cmd_path" <<EOF
#!/bin/bash
echo "\$0 called with arguments: \$@" >> "$log_file"
echo "$response"
exit $exit_code
EOF
  chmod +x "$cmd_path"
}

# Create a mock command that returns different responses based on arguments
create_conditional_mock() {
  local cmd_path="$1"
  local condition_file="$2"
  
  mkdir -p "$(dirname "$cmd_path")"
  
  cat > "$cmd_path" <<EOF
#!/bin/bash
source "$condition_file"

# Process arguments
args="\$*"
for condition in "\${CONDITIONS[@]}"; do
  pattern=\$(echo "\$condition" | cut -d ':' -f 1)
  response=\$(echo "\$condition" | cut -d ':' -f 2)
  exit_code=\$(echo "\$condition" | cut -d ':' -f 3)
  
  if [[ "\$args" =~ \$pattern ]]; then
    echo "\$response"
    exit \$exit_code
  fi
done

# Default behavior
echo "\${DEFAULT_RESPONSE:-No matching condition found}"
exit \${DEFAULT_EXIT_CODE:-1}
EOF
  chmod +x "$cmd_path"
}

# Create a mock lsblk command with configurable JSON output
create_mock_lsblk() {
  local cmd_path="$1"
  local json_output="$2"
  
  mkdir -p "$(dirname "$cmd_path")"
  
  cat > "$cmd_path" <<EOF
#!/bin/bash
if [[ "\$*" == *"--json"* ]]; then
  cat <<'JSON_OUTPUT'
$json_output
JSON_OUTPUT
else
  echo "NAME MAJ:MIN RM SIZE RO TYPE MOUNTPOINT"
  echo "nvme0n1 259:0 0 100G 0 disk"
fi
exit 0
EOF
  chmod +x "$cmd_path"
}

# Create a condition file for conditional mocks
create_condition_file() {
  local file_path="$1"
  shift
  local conditions=("$@")
  
  mkdir -p "$(dirname "$file_path")"
  
  cat > "$file_path" <<EOF
#!/bin/bash
CONDITIONS=(
EOF

  for condition in "${conditions[@]}"; do
    echo "  \"$condition\"" >> "$file_path"
  done

  cat >> "$file_path" <<EOF
)

# Default response if no pattern matches
DEFAULT_RESPONSE="Default response"
DEFAULT_EXIT_CODE=0
EOF
}
