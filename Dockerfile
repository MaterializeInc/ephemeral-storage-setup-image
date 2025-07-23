# Copyright Materialize, Inc. and contributors. All rights reserved.
#
# Use of this software is governed by the Business Source License
# included in the LICENSE file at the root of this repository.
#
# As of the Change Date specified in that file, in accordance with
# the Business Source License, use of this software will be governed
# by the Apache License, Version 2.0.

FROM --platform=$BUILDPLATFORM rust:1.88.0-alpine3.22 AS builder
ARG BUILDARCH
ARG TARGETARCH

# Installing binutils-${host_arch} conflicts with binutils, as they install the same files.
RUN TARGET_RUSTARCH="$(echo "${TARGETARCH}" | sed 's/arm64/aarch64/; s/amd64/x86_64/')"; \
    TARGET="${TARGET_RUSTARCH}-unknown-linux-musl"; \
    ADDITIONAL_BINUTILS="$(if [ "${BUILDARCH}" != "${TARGETARCH}" ]; then echo "binutils-${TARGET_RUSTARCH}"; fi)"; \
    apk add --no-cache musl-dev "${ADDITIONAL_BINUTILS}" \
    && rustup target add "${TARGET}" \
    && rustup toolchain add --force-non-host "$(cargo version | awk '{print $2}')-${TARGET}"

WORKDIR /build
COPY src ./src
COPY Cargo.toml ./
COPY Cargo.lock ./

# For some reason, setting the linker for the host arch breaks things, even when not cross compiling.
# Setting the linker for the cross compilation arch is required, though.
RUN RUSTARCH="$(echo "${TARGETARCH}" | sed 's/arm64/aarch64/; s/amd64/x86_64/')"; \
    TARGET="${RUSTARCH}-unknown-linux-musl"; \
    if [ "${BUILDARCH}" != "${TARGETARCH}" ]; then export CARGO_TARGET_$(echo "${RUSTARCH}" | tr '[a-z]' '[A-Z]')_UNKNOWN_LINUX_MUSL_LINKER="/usr/${RUSTARCH}-alpine-linux-musl/bin/ld"; fi; \
    cargo build --release --target "${TARGET}" \
    && mv "target/${TARGET}/release/ephemeral-storage-setup" ./


FROM alpine:3.22 AS final

# keeping bash for now just for debugging
RUN apk add --no-cache \
    lvm2 \
    lsblk \
    bash \
    kubectl

COPY --from=builder /build/ephemeral-storage-setup /usr/local/bin/
CMD ["ephemeral-storage-setup"]
