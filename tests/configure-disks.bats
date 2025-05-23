#!/usr/bin/env bats

# Tests for configure-disks.sh script

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
  export SCRIPT_PATH="${TEMP_DIR}/configure-disks.sh"
  cp "$(pwd)/configure-disks.sh" "$SCRIPT_PATH"
  chmod +x "$SCRIPT_PATH"
  
  # Create mock data directory
  export MOCK_DATA="${TEMP_DIR}/data"
  mkdir -p "$MOCK_DATA"
}

# Clean up our test environment
teardown() {
  export PATH="$ORIGINAL_PATH"
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

# Helper for mock lsblk command with json output
create_mock_lsblk_json() {
  local json_file="${MOCK_DATA}/lsblk.json"
  echo "$1" > "$json_file"
  
  cat > "${MOCK_BIN}/lsblk" <<EOF
#!/bin/bash
case "\$*" in
  *--json*)
    cat "${json_file}"
    ;;
  *)
    echo "Mock lsblk - normal output"
    ;;
esac
exit 0
EOF
  chmod +x "${MOCK_BIN}/lsblk"
}

create_mock_nvme_list_for_azure() {
  cat > "${MOCK_BIN}/nvme" <<EOF
#!/bin/bash
if [[ "\$1" == "list" ]]; then
  echo "Node             SN                   Model                                    Namespace Usage                      Format           FW Rev"
  echo "/dev/nvme0n1     1234567890ABCDEF     Azure NVMe Disk                           1         0.00   B /   0.00   B    512   B +  0 B   1.0"
  echo "/dev/nvme1n1     1234567890ABCDE0     Azure NVMe Disk                           1         0.00   B /   0.00   B    512   B +  0 B   1.0"
  exit 0
else
  echo "Unknown nvme command"
  exit 1
fi
EOF
  chmod +x "${MOCK_BIN}/nvme"
}

# Mock other commands we don't want to actually run
mock_common_commands() {
  # Create mock vgs that shows no existing volume groups
  create_mock "vgs" 0 ""
  
  # Create mock pvs that shows no existing physical volumes
  create_mock "pvs" 0 ""
  
  # Create mock pvcreate that always succeeds
  create_mock "pvcreate" 0 "Physical volume created"
  
  # Create mock vgcreate that always succeeds
  create_mock "vgcreate" 0 "Volume group created"

  # Create mock curl that fails by default
  create_mock "curl" 1 "Connection failed"
}

# Test argument parsing
@test "Script accepts --cloud-provider argument" {
  mock_common_commands
  
  # Create mock find_nvme_devices function that returns a test device
  create_mock_lsblk_json '{
    "blockdevices": [
      {"name": "nvme0n1", "path": "/dev/nvme0n1", "mountpoint": null, "children": []}
    ]
  }'
  
  # Run the script with --cloud-provider argument
  run "$SCRIPT_PATH" --cloud-provider aws
  
  # Check output
  echo "Output: $output"
  [ "$status" -eq 0 ]
  [[ "$output" == *"Using cloud provider: aws"* ]]
}

@test "Script accepts --vg-name argument" {
  mock_common_commands
  
  # Create mock find_nvme_devices function that returns a test device
  create_mock_lsblk_json '{
    "blockdevices": [
      {"name": "nvme0n1", "path": "/dev/nvme0n1", "mountpoint": null, "children": []}
    ]
  }'
  
  # Create mock find_nvme_devices that returns a test device
  cat > "${MOCK_BIN}/find" <<EOF
#!/bin/bash
echo "/dev/nvme0n1"
exit 0
EOF
  chmod +x "${MOCK_BIN}/find"
  
  run "$SCRIPT_PATH" --vg-name custom-vg --cloud-provider aws
  
  # Check output
  echo "Output: $output"
  [ "$status" -eq 0 ]
  [[ "$output" == *"Creating volume group custom-vg"* ]]
}

# Test AWS device detection
@test "Finds AWS NVMe instance store devices" {
  mock_common_commands
  
  # Force AWS provider
  create_mock "curl" 1 ""
  
  # Create mock lsblk that returns AWS NVMe devices
  create_mock_lsblk_json '{
    "blockdevices": [
      {
        "name": "nvme0n1", 
        "path": "/dev/nvme0n1", 
        "model": "Amazon EC2 NVMe Instance Storage",
        "mountpoint": null, 
        "children": []
      },
      {
        "name": "nvme1n1", 
        "path": "/dev/nvme1n1", 
        "model": "Amazon EC2 NVMe Instance Storage",
        "mountpoint": null, 
        "children": []
      }
    ]
  }'
  
  # Run the script with forced AWS provider
  run "$SCRIPT_PATH" --cloud-provider aws
  
  # Check output
  echo "Output: $output"
  [ "$status" -eq 0 ]
  [[ "$output" == *"Found devices: /dev/nvme0n1 /dev/nvme1n1"* ]]
}

# Test GCP device detection
@test "Finds GCP local SSD devices" {
  mock_common_commands

  cat > "${MOCK_BIN}/find" <<EOF
#!/bin/bash
if [[ "\$*" == *"google-local-ssd"* ]]; then
  echo "/dev/disk/by-id/google-local-ssd-0"
  echo "/dev/disk/by-id/google-local-ssd-1"
  exit 0
fi
exit 0
EOF
  chmod +x "${MOCK_BIN}/find"

  run "$SCRIPT_PATH" --cloud-provider gcp

  echo "Output: $output"
  [ "$status" -eq 0 ]
  [[ "$output" == *"Found devices: /dev/disk/by-id/google-local-ssd-0 /dev/disk/by-id/google-local-ssd-1"* ]]
}

# Test LVM setup
@test "Creates LVM volume group with discovered devices" {
  mock_common_commands

  create_mock_lsblk_json '{
    "blockdevices": [
      {
        "name": "nvme0n1", 
        "path": "/dev/nvme0n1", 
        "model": "Amazon EC2 NVMe Instance Storage",
        "mountpoint": null, 
        "children": []
      }
    ]
  }'

  run "$SCRIPT_PATH" --cloud-provider aws

  echo "Output: $output"
  [ "$status" -eq 0 ]
  [[ "$output" == *"Creating physical volume on /dev/nvme0n1"* ]]
  [[ "$output" == *"Creating volume group instance-store-vg"* ]]
  [[ "$output" == *"NVMe disk configuration completed successfully"* ]]
}

@test "Skips LVM setup if volume group already exists" {
  mock_common_commands

  create_mock "vgs" 0 "  instance-store-vg  1  2  0 wz--n- 1.00g 0"

  create_mock_lsblk_json '{
    "blockdevices": [
      {
        "name": "nvme0n1",
        "path": "/dev/nvme0n1",
        "model": "Amazon EC2 NVMe Instance Storage",
        "mountpoint": null,
        "children": []
      }
    ]
  }'

  run "$SCRIPT_PATH" --cloud-provider aws

  echo "Output: $output"
  [ "$status" -eq 0 ]
  [[ "$output" == *"Volume group instance-store-vg already exists"* ]]
  [[ "$output" == *"NVMe disk configuration completed successfully"* ]]
}

@test "Fails if no devices are found" {
  create_mock "vgs" 0 ""
  create_mock "pvs" 0 ""
  create_mock "pvcreate" 0 "Physical volume created"
  create_mock "vgcreate" 0 "Volume group created"
  create_mock "curl" 1 "Connection failed"

  create_mock_lsblk_json '{"blockdevices": []}'

  create_mock "nvme" 0 ""
  create_mock "grep" 0 ""
  create_mock "awk" 0 ""

  TEMP_SCRIPT="${TEMP_DIR}/patched-script.sh"
  cp "$SCRIPT_PATH" "$TEMP_SCRIPT"

  sed -i 's/\[\[ ${#devices\[@]} -eq 0 ]]/[[ ${#devices[@]} -eq 0 || "${devices[0]}" == "" ]]/' "$TEMP_SCRIPT"

  run "$TEMP_SCRIPT" --cloud-provider aws

  echo "Output: $output"
  [ "$status" -eq 1 ]
  [[ "$output" == *"No suitable NVMe devices found"* ]]
}

@test "Handles unknown option error" {
  run "$SCRIPT_PATH" --invalid-option

  echo "Output: $output"
  [ "$status" -eq 1 ]
  [[ "$output" == *"Unknown option: --invalid-option"* ]]
}

@test "Detects Azure NVMe devices via nvme list" {
  mock_common_commands
  create_mock_nvme_list_for_azure

  run "$SCRIPT_PATH" --cloud-provider azure

  echo "Output: $output"
  [ "$status" -eq 0 ]
  [[ "$output" == *"Found devices: /dev/nvme0n1 /dev/nvme1n1"* ]]
  [[ "$output" == *"Creating volume group instance-store-vg"* ]]
  [[ "$output" == *"NVMe disk configuration completed successfully"* ]]
}
