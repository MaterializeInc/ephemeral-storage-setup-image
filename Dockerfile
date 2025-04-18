# Copyright Materialize, Inc. and contributors. All rights reserved.
#
# Use of this software is governed by the Business Source License
# included in the LICENSE file at the root of this repository.
#
# As of the Change Date specified in that file, in accordance with
# the Business Source License, use of this software will be governed
# by the Apache License, Version 2.0.

FROM alpine:3.21

RUN apk add --no-cache \
    nvme-cli \
    lvm2 \
    lsblk \
    bash \
    jq \
    curl \
    kubectl

# Disk configuration script
COPY configure-disks.sh /usr/local/bin/configure-disks.sh
# Taint removal script
COPY remove-taint.sh /usr/local/bin/remove-taint.sh

RUN chmod +x /usr/local/bin/configure-disks.sh /usr/local/bin/remove-taint.sh
