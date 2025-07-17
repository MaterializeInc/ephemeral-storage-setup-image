#!/usr/bin/env bash

# Helper functions for testing shell scripts


# Create a temporary directory for our test environment
setup() {
    # Create temp directory for test artifacts
    TEMP_DIR="$(mktemp -d)"
    export TEMP_DIR

    # Save original path
    export ORIGINAL_PATH="$PATH"

    # Create mock bin directory for our mock commands
    export MOCK_BIN="${TEMP_DIR}/bin"
    mkdir -p "${MOCK_BIN}"

    # Add our mock bin to the front of the PATH
    export PATH="${MOCK_BIN}:$PATH"

    # Copy the script for testing
    export LVM_SCRIPT_PATH="${MOCK_BIN}/configure-lvm.sh"
    export SWAP_SCRIPT_PATH="${MOCK_BIN}/configure-swap.sh"
    export REMOVE_TAINT_SCRIPT_PATH="${MOCK_BIN}/remove-taint.sh"
    cp "$(dirname "$0")/../configure-lvm.sh" "${MOCK_BIN}/"
    cp "$(dirname "$0")/../configure-swap.sh" "${MOCK_BIN}/"
    cp "$(dirname "$0")/../detect-disks.sh" "${MOCK_BIN}/"
    cp "$(dirname "$0")/../remove-taint.sh" "${MOCK_BIN}/"

    # Create mock data directory
    export MOCK_DATA="${TEMP_DIR}/data"
    mkdir -p "$MOCK_DATA"

    # Set required environment variables
    export NODE_NAME="test-node"
}

# Clean up our test environment
teardown() {
    export PATH="${ORIGINAL_PATH}"
    rm -rf "${TEMP_DIR}"
}

assert_equal() {
    local output="$1"
    local expected_output="$2"
    if [[ "${output}" != "${expected_output}" ]]; then
        echo "'${output}' != '${expected_output}'" >&2
        exit 1
    fi
}

assert_output_contains() {
    local output="$1"
    local expected_output="$2"
    if [[ "${output}" != *"${expected_output}"* ]]; then
        echo "'${expected_output}' not found in '${output}'" >&2
        exit 1
    fi
}

assert_output_not_contains() {
    local output="$1"
    local expected_output="$2"
    if [[ "${output}" == *"${expected_output}"* ]]; then
        echo "'${expected_output}' found in '${output}' when it should not be" >&2
        exit 1
    fi
}

run_test() {
    local test_func="$1"

    setup

    echo "Running test: ${test_func}"
    "${test_func}"

    teardown
}

run_expecting_rc() {
    local expected_rc="$1"
    shift 1

    set +e
    "${@}"
    rc="$?"
    set -e

    assert_equal "${rc}" "${expected_rc}"
}

# Helper to create mock commands
create_mock() {
    local cmd="$1"
    local exit_code="${2:-0}"
    local output="$3"

    cat > "${MOCK_BIN}/${cmd}" <<EOF
#!/bin/bash
set -euo pipefail
echo "${output}"
exit ${exit_code}
EOF
    chmod +x "${MOCK_BIN}/${cmd}"
}

create_mock_detect_disks() {
    local rc="$1"
    local output="$2"
    cat > "${MOCK_BIN}/detect-disks.sh" <<EOF
#!/bin/bash
set -euo pipefail
find_nvme_devices() {
    echo "${output[@]}"
    exit "$rc"
}
EOF
    chmod +x "${MOCK_BIN}/detect-disks.sh"
}

# Helper for mock lsblk command with json output
create_mock_lsblk_json() {
    local json_file="${MOCK_DATA}/lsblk.json"
    echo "$1" > "$json_file"

    cat > "${MOCK_BIN}/lsblk" <<EOF
#!/bin/bash
set -euo pipefail
case "\$*" in
    *--json*)
        cat "${json_file}"
        ;;
    *)
        echo "Mock lsblk - normal output"
    ;;
esac
EOF
    chmod +x "${MOCK_BIN}/lsblk"
}

create_mock_nvme_list_for_azure() {
    cat > "${MOCK_BIN}/nvme" <<EOF
#!/bin/bash
set -euo pipefail
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
