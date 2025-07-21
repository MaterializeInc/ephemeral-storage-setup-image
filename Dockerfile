# Copyright Materialize, Inc. and contributors. All rights reserved.
#
# Use of this software is governed by the Business Source License
# included in the LICENSE file at the root of this repository.
#
# As of the Change Date specified in that file, in accordance with
# the Business Source License, use of this software will be governed
# by the Apache License, Version 2.0.

FROM rust:1.88.0-alpine3.22 AS builder
RUN apk add --no-cache musl-dev

WORKDIR /ephemeral-storage-setup
COPY src ./src
COPY Cargo.toml ./
COPY Cargo.lock ./
RUN cargo build --release

FROM alpine:3.22 AS final

# keeping bash for now just for debugging
RUN apk add --no-cache \
    lvm2 \
    lsblk \
    bash \
    kubectl

COPY --from=builder /ephemeral-storage-setup/target/release/ephemeral-storage-setup /usr/local/bin/
CMD ["ephemeral-storage-setup"]
