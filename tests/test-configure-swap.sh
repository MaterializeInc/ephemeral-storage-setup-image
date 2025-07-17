#!/usr/bin/env bash

# Tests for configure-lvm.sh script

set -euo pipefail

source "$(dirname "$0")/test_helpers.sh"


mock_swapon() {
  local pre_existing_names="$1"
  local final_show_output="$2"

  cat > "${MOCK_BIN}/swapon" <<EOF
#!/bin/bash
set -euo pipefail
case "\$*" in
    *--show=NAME*)
        echo "${pre_existing_names}"
        ;;
    *--show*)
        echo "${final_show_output}"
        ;;
    *)
        # No output in other cases
        ;;
esac
EOF
  chmod +x "${MOCK_BIN}/swapon"
}

mock_sysctl() {
    cat > "${MOCK_BIN}/sysctl" <<'EOF'
#!/bin/bash
set -euo pipefail
# Add spaces around the =, to match the actual sysctl output
echo "${1%%=*} = ${1##*=}"
EOF
    chmod +x "${MOCK_BIN}/sysctl"
}

mock_swap_commands() {
    create_mock_detect_disks 0 "/dev/nvme0n1
/dev/nvme1n1"
    mock_swapon "" "NAME      TYPE      SIZE USED PRIO
/dev/nvme0n1   1G   0B   -2
/dev/nvme1n1   1G   0B   -2"
    mock_sysctl
    create_mock "mkswap" 0 "Setting up swapspace version 1, size = 1024 MiB (1073737728 bytes)
no label, UUID=e6fd39dc-2fcc-4584-a3a2-436494a4944e"
}

test_requires_cloud_provider_argument() {
    mock_swap_commands

    local output
    output="$(run_expecting_rc 1 "${SWAP_SCRIPT_PATH}")"
    assert_output_contains "${output}" "ERROR: Cloud provider not specified."

    output="$(run_expecting_rc 0 "${SWAP_SCRIPT_PATH}" --cloud-provider aws)"
    assert_output_contains "${output}" "Using cloud provider: aws"
}

test_it_configures_swap_and_sysctls() {
    mock_swap_commands

    local output
    output="$(run_expecting_rc 0 "${SWAP_SCRIPT_PATH}" --cloud-provider aws)"
    assert_output_contains "${output}" "Running mkswap on /dev/nvme0n1"
    assert_output_contains "${output}" "Running swapon on /dev/nvme0n1"
    assert_output_contains "${output}" "Running mkswap on /dev/nvme1n1"
    assert_output_contains "${output}" "Running swapon on /dev/nvme1n1"
    assert_output_contains "${output}" "vm.swappiness"
    assert_output_contains "${output}" "vm.min_free_kbytes"
    assert_output_contains "${output}" "vm.watermark_scale_factor"
}

test_it_skips_existing_swap_devices() {
    mock_swap_commands
    mock_swapon "/dev/nvme0n1
/dev/nvme1n1" "NAME      TYPE      SIZE USED PRIO
/dev/nvme0n1   1G   0B   -2
/dev/nvme1n1   1G   0B   -2"

    local output
    output="$(run_expecting_rc 0 "${SWAP_SCRIPT_PATH}" --cloud-provider aws)"
    assert_output_not_contains "${output}" "Running mkswap"
    assert_output_not_contains "${output}" "Running swapon"
    assert_output_contains "${output}" "vm.swappiness"
    assert_output_contains "${output}" "vm.min_free_kbytes"
    assert_output_contains "${output}" "vm.watermark_scale_factor"
}

run_test test_requires_cloud_provider_argument
run_test test_it_configures_swap_and_sysctls
run_test test_it_skips_existing_swap_devices
