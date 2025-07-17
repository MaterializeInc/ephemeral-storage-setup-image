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
    kubectl

# Disk detection library functions
COPY detect-disks.sh /usr/local/bin/detect-disks.sh
# LVM configuration script
COPY configure-lvm.sh /usr/local/bin/configure-lvm.sh
# Symlink for backwards compatibility with the old name for configure-lvm.sh
RUN ln -s /usr/local/bin/configure-lvm.sh /usr/local/bin/configure-disk.sh
# Swap configuration script
COPY configure-swap.sh /usr/local/bin/configure-swap.sh
# Taint removal script
COPY remove-taint.sh /usr/local/bin/remove-taint.sh

RUN chmod +x \
    /usr/local/bin/configure-lvm.sh \
    /usr/local/bin/configure-swap.sh \
    /usr/local/bin/remove-taint.sh
