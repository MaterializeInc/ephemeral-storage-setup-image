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

find_aws_standard_devices() {
    lsblk --json --output-all | \
        jq -r '.blockdevices[] | select(.model // empty | contains("Amazon EC2 NVMe Instance Storage")) | .path'
}

find_aws_bottlerocket_devices() {
    find_aws_standard_devices | sed 's#^#/.bottlerocket/rootfs#'
}

find_aws_devices() {
    # Check if running in Bottlerocket
    if [[ -d "/.bottlerocket" ]]; then
        find_aws_bottlerocket_devices
    else
        find_aws_standard_devices
    fi
}

find_gcp_devices() {
    find /dev/disk/by-id/ -name "google-local-ssd-*"
}

find_azure_devices() {
    nvme list | awk '/NVMe/ {print $1}'
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
            find_aws_devices
            ;;
        gcp)
            find_gcp_devices
            ;;
        azure)
            find_azure_devices
            ;;
        *)
            # Generic approach for any other cloud or environment
            find_generic_devices
            ;;
    esac
}
