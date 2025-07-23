# Copyright Materialize, Inc. and contributors. All rights reserved.
#
# Use of this software is governed by the Business Source License
# included in the LICENSE file at the root of this repository.
#
# As of the Change Date specified in that file, in accordance with
# the Business Source License, use of this software will be governed
# by the Apache License, Version 2.0.

FROM --platform=$BUILDPLATFORM tonistiigi/xx:1.6.1 AS xx

FROM --platform=$BUILDPLATFORM rust:1.88.0-alpine3.22 AS builder
COPY --from=xx / /

ARG BUILDARCH
ARG TARGETARCH
ARG BUILDPLATFORM
ARG TARGETPLATFORM
RUN xx-apk add --no-cache xx-c-essentials && \
    apk add --no-cache musl-dev clang perl make
# Install target for cargo and other build tools (ie: clang)
RUN xx-cargo --setup-target-triple

WORKDIR /build
COPY src ./src
COPY Cargo.toml ./
COPY Cargo.lock ./
RUN xx-cargo build --release && \
    mv ./target/$(xx-cargo --print-target-triple)/release/ephemeral-storage-setup ./ && \
    xx-verify --static ./ephemeral-storage-setup


FROM alpine:3.22 AS final

# keeping bash for now just for debugging
RUN apk add --no-cache \
    lvm2 \
    lsblk \
    bash \
    kubectl

COPY --from=builder /build/ephemeral-storage-setup /usr/local/bin/
CMD ["ephemeral-storage-setup"]
