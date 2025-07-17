#!/usr/bin/env bash

# Tests for detect-disks.sh script

set -euo pipefail

source "$(dirname "$0")/test_helpers.sh"
source "$(dirname "$0")/../detect-disks.sh"
# Hide intermediate output, as it just makes the output messy.
set +x

test_find_aws_bottlerocket_devices() {
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
    local expected_output="/.bottlerocket/rootfs/dev/nvme0n1
/.bottlerocket/rootfs/dev/nvme1n1"

    local output="$(run_expecting_rc 0 find_aws_bottlerocket_devices)"
    assert_equal "${output}" "${expected_output}"
}

test_find_aws_standard_devices() {
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
    local expected_output="/dev/nvme0n1
/dev/nvme1n1"

    local output="$(run_expecting_rc 0 find_aws_standard_devices)"
    assert_equal "${output}" "${expected_output}"
}

test_find_gcp_devices() {
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

    local expected_output="/dev/disk/by-id/google-local-ssd-0
/dev/disk/by-id/google-local-ssd-1"

    local output="$(run_expecting_rc 0 find_gcp_devices)"
    assert_equal "${output}" "${expected_output}"
}

test_find_azure_devices() {
    create_mock_nvme_list_for_azure

    local expected_output="/dev/nvme0n1
/dev/nvme1n1"

    local output="$(run_expecting_rc 0 find_azure_devices)"
    assert_equal "${output}" "${expected_output}"
}

test_find_generic_devices() {
    # Create mock lsblk that returns AWS NVMe devices
    create_mock_lsblk_json '{
      "blockdevices": [
        {
          "name": "nvme0n1",
          "path": "/dev/nvme0n1",
          "model": "Generic",
          "mountpoint": null,
          "children": []
        },
        {
          "name": "nvme1n1",
          "path": "/dev/nvme1n1",
          "model": "Generic",
          "mountpoint": "/",
          "children": []
        },
        {
          "name": "nvme2n1",
          "path": "/dev/nvme2n1",
          "model": "Generic",
          "mountpoint": null,
          "children": ["asdf"]
        },
        {
          "name": "nvme3n1",
          "path": "/dev/nvme3n1",
          "model": "Generic",
          "mountpoint": null,
          "children": []
        }
      ]
    }'
    local expected_output="/dev/nvme0n1
/dev/nvme3n1"

    local output="$(run_expecting_rc 0 find_generic_devices)"
    assert_equal "${output}" "${expected_output}"
}


run_test test_find_aws_bottlerocket_devices
run_test test_find_aws_standard_devices
run_test test_find_gcp_devices
run_test test_find_azure_devices
run_test test_find_generic_devices
