#!/usr/bin/env bash

# Tests for configure-lvm.sh script

set -euo pipefail

source "$(dirname "$0")/test_helpers.sh"

# Mock lvm commands we don't want to actually run
mock_lvm_commands() {
  # Create mock vgs that shows no existing volume groups
  create_mock "vgs" 0 ""

  # Create mock pvs that shows no existing physical volumes
  create_mock "pvs" 0 ""

  # Create mock pvcreate that always succeeds
  create_mock "pvcreate" 0 "Physical volume created"

  # Create mock vgcreate that always succeeds
  create_mock "vgcreate" 0 "Volume group created"
}

# Test argument parsing
test_accepts_cloud_provider_argument() {
  mock_lvm_commands
  create_mock_detect_disks 0 "/dev/nvme0n1
/dev/nvme1n1"

  local output="$(run_expecting_rc 0 "${LVM_SCRIPT_PATH}" --cloud-provider aws)"
  assert_output_contains "${output}" "Using cloud provider: aws"
}

test_accepts_vg_name_argument() {
  mock_lvm_commands
  create_mock_detect_disks 0 "/dev/nvme0n1
/dev/nvme1n1"

  local output="$(run_expecting_rc 0 "${LVM_SCRIPT_PATH}" --cloud-provider aws --vg-name custom-vg)"
  assert_output_contains "${output}" "Creating volume group custom-vg"
}

test_creates_physical_volumes_and_volume_group() {
  mock_lvm_commands
  create_mock_detect_disks 0 "/dev/nvme0n1
/dev/nvme1n1"

  local output="$(run_expecting_rc 0 "${LVM_SCRIPT_PATH}" --cloud-provider aws)"
  assert_output_contains "${output}" "Creating physical volume on /dev/nvme0n1"
  assert_output_contains "${output}" "Creating physical volume on /dev/nvme1n1"
  assert_output_contains "${output}" "Creating volume group instance-store-vg"
  assert_output_contains "${output}" "NVMe disk configuration completed successfully"
}

test_skips_existing_volume_group() {
  mock_lvm_commands
  create_mock_detect_disks 0 "/dev/nvme0n1
/dev/nvme1n1"
  create_mock "vgs" 0 "  instance-store-vg  1  2  0 wz--n- 1.00g 0"

  local output="$(run_expecting_rc 0 "${LVM_SCRIPT_PATH}" --cloud-provider aws)"
  assert_output_contains "${output}" "Volume group instance-store-vg already exists"
  assert_output_contains "${output}" "NVMe disk configuration completed successfully"
}

test_fails_if_no_devices() {
  mock_lvm_commands
  create_mock_detect_disks 1 ""

  local output="$(run_expecting_rc 1 "${LVM_SCRIPT_PATH}" --cloud-provider aws)"

  assert_output_contains "${output}" "No suitable NVMe devices found"
}

test_errors_on_invalid_option() {
  mock_lvm_commands

  local output="$(run_expecting_rc 1 "${LVM_SCRIPT_PATH}" --invalid-option)"

  assert_output_contains "${output}" "Unknown option: --invalid-option"
}


run_test test_accepts_cloud_provider_argument
run_test test_accepts_vg_name_argument
run_test test_creates_physical_volumes_and_volume_group
run_test test_skips_existing_volume_group
run_test test_fails_if_no_devices
run_test test_errors_on_invalid_option
