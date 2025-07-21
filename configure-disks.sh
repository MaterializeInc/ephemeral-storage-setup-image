#!/usr/bin/env bash
set -euo pipefail

# This is just a simple shim for backwards compatibility.
# It is recommended to call ephemeral-storage-setup directly.
ephemeral-storage-setup lvm "$@"
