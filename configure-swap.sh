#!/usr/bin/env bash

# Copyright Materialize, Inc. and contributors. All rights reserved.
#
# Use of this software is governed by the Business Source License
# included in the LICENSE file at the root of this repository.
#
# As of the Change Date specified in that file, in accordance with
# the Business Source License, use of this software will be governed
# by the Apache License, Version 2.0.

set -euo pipefail

# import functions for detecting devices
source "$(dirname "$0")/detect-disks.sh"

CLOUD_PROVIDER=""

while [[ $# -gt 0 ]]; do
  case $1 in
    --cloud-provider|-c)
      CLOUD_PROVIDER="$2"
      shift 2
      ;;
    --help|-h)
      echo "Usage: $0 [options]"
      echo "Options:"
      echo "  --cloud-provider, -c PROVIDER   Specify cloud provider (aws, gcp, azure, generic)"
      echo "  --help, -h             Show this help message"
      exit 0
      ;;
    *)
      echo "Unknown option: $1"
      exit 1
      ;;
  esac
done

# Validate required parameters
if [[ -z "$CLOUD_PROVIDER" ]]; then
    echo "ERROR: Cloud provider not specified. Please provide using the --cloud-provider option."
    echo "Valid options: aws, gcp, azure, generic"
    exit 1
fi

# Initialize swap on discovered devices
setup_swap() {
    local -a devices=("$@")

    if [[ ${#devices[@]} -eq 0 ]]; then
        echo "No suitable NVMe devices found"
        exit 1
    fi

    echo "Found devices: ${devices[*]}"

    # Enable swap on each device
    for device in "${devices[@]}"; do
        if ! swapon --show=NAME --noheadings | grep -q "$device"; then
            echo "Running mkswap on $device"
            mkswap "$device"
            echo "Running swapon on $device"
            swapon "$device"
        fi
    done

    echo "Setting sysctl parameters to increase swap use and safety."
    sysctl vm.swappiness=100
    sysctl vm.min_free_kbytes=1048576
    sysctl vm.watermark_scale_factor=100

    echo "Swap setup completed successfully"
    swapon --show

    return 0
}

echo "Starting NVMe disk configuration..."
echo "Using cloud provider: $CLOUD_PROVIDER"

# Find NVMe devices
mapfile -t NVME_DEVICES < <(find_nvme_devices "$CLOUD_PROVIDER")

# Setup swap
if setup_swap "${NVME_DEVICES[@]}"; then
    echo "NVMe disk configuration completed successfully"
    exit 0
else
    echo "NVMe disk configuration failed"
    exit 1
fi
