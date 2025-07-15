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

find_aws_bottlerocket_devices() {
    local nvme_devices=()
    local BOTTLEROCKET_ROOT="/.bottlerocket/rootfs"

    mapfile -t SSD_NVME_DEVICE_LIST < <(lsblk --json --output-all | \
        jq -r '.blockdevices[] | select(.model // empty | contains("Amazon EC2 NVMe Instance Storage")) | .path')

    for device in "${SSD_NVME_DEVICE_LIST[@]}"; do
        nvme_devices+=("$BOTTLEROCKET_ROOT$device")
    done

    echo "${nvme_devices[@]}"
}

find_aws_standard_devices() {
    lsblk --json --output-all | \
        jq -r '.blockdevices[] | select(.model // empty | contains("Amazon EC2 NVMe Instance Storage")) | .path'
}

find_aws_devices() {
    local nvme_devices=()

    # Check if running in Bottlerocket
    if [[ -d "/.bottlerocket" ]]; then
        mapfile -t nvme_devices < <(find_aws_bottlerocket_devices)
    else
        mapfile -t nvme_devices < <(find_aws_standard_devices)
    fi

    echo "${nvme_devices[@]}"
}

find_gcp_devices() {
    local ssd_devices=()

    # Check for Google Local SSD devices
    local devices
    devices=$(find /dev/disk/by-id/ -name "google-local-ssd-*" 2>/dev/null || true)

    if [ -n "$devices" ]; then
        while read -r device; do
            ssd_devices+=("$device")
        done <<< "$devices"
    fi

    echo "${ssd_devices[@]}"
}

find_azure_devices() {
    local azure_disks=()

    mapfile -t azure_disks < <(nvme list | grep "NVMe" | awk '{print $1}' || true)

    echo "${azure_disks[@]}"
}

find_generic_devices() {
    lsblk --json --output-all | \
        jq -r '.blockdevices[] | select(.name | startswith("nvme")) | select(.mountpoint == null and (.children | length == 0)) | .path'
}

find_nvme_devices() {
    local cloud=$1
    local nvme_devices=()

    case $cloud in
        aws)
            mapfile -t nvme_devices < <(find_aws_devices)
            ;;
        gcp)
            mapfile -t nvme_devices < <(find_gcp_devices)
            ;;
        azure)
            mapfile -t nvme_devices < <(find_azure_devices)
            ;;
        *)
            # Generic approach for any other cloud or environment
            mapfile -t nvme_devices < <(find_generic_devices)
            ;;
    esac

    echo "${nvme_devices[@]}"
}
