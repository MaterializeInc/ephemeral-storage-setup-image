#!/usr/bin/env bash

# Copyright Materialize, Inc. and contributors. All rights reserved.
#
# Use of this software is governed by the Business Source License
# included in the LICENSE file at the root of this repository.
#
# As of the Change Date specified in that file, in accordance with
# the Business Source License, use of this software will be governed
# by the Apache License, Version 2.0.

set -xeuo pipefail

# import functions for detecting devices
source "$(dirname "$0")/detect-disks.sh"

VG_NAME="instance-store-vg"
CLOUD_PROVIDER=""

while [[ $# -gt 0 ]]; do
  case $1 in
    --cloud-provider|-c)
      CLOUD_PROVIDER="$2"
      shift 2
      ;;
    --vg-name|-v)
      VG_NAME="$2"
      shift 2
      ;;
    --help|-h)
      echo "Usage: $0 [options]"
      echo "Options:"
      echo "  --cloud-provider, -c PROVIDER   Specify cloud provider (aws, gcp, azure, generic)"
      echo "  --vg-name, -v NAME     Specify volume group name (default: instance-store-vg)"
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

# Initialize LVM on discovered devices
setup_lvm() {
    local -a devices=("$@")

    if [[ ${#devices[@]} -eq 0 ]]; then
        echo "No suitable NVMe devices found"
        exit 1
    fi

    echo "Found devices: ${devices[*]}"

    # Check if volume group already exists
    if vgs | grep -q "$VG_NAME"; then
        echo "Volume group $VG_NAME already exists"
        return 0
    fi

    # Create physical volumes
    for device in "${devices[@]}"; do
        if ! pvs | grep -q "$device"; then
            echo "Creating physical volume on $device"
            pvcreate -f "$device"
        fi
    done

    # Create volume group with all devices
    echo "Creating volume group $VG_NAME"
    vgcreate "$VG_NAME" "${devices[@]}"

    # Display results
    pvs
    vgs

    echo "LVM setup completed successfully"
    return 0
}

echo "Starting NVMe disk configuration..."
echo "Using cloud provider: $CLOUD_PROVIDER"

# Find NVMe devices
mapfile -t NVME_DEVICES < <(find_nvme_devices "$CLOUD_PROVIDER")

# Setup LVM
if setup_lvm "${NVME_DEVICES[@]}"; then
    echo "NVMe disk configuration completed successfully"
    exit 0
else
    echo "NVMe disk configuration failed"
    exit 1
fi
