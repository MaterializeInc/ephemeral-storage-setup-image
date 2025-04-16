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

TAINT_KEY="disk-unconfigured"

if [ -z "${NODE_NAME:-}" ]; then
    echo "Error: NODE_NAME environment variable is required but not set"
    exit 1
fi

echo "Starting taint management for node: $NODE_NAME"
echo "Action: $1"

# Check if necessary environment variables and files exist
if [ -z "$KUBERNETES_SERVICE_HOST" ] || [ -z "$KUBERNETES_SERVICE_PORT" ]; then
    echo "Error: Kubernetes service environment variables not found"
    exit 1
fi

if [ ! -f "/var/run/secrets/kubernetes.io/serviceaccount/token" ]; then
    echo "Error: Service account token not found"
    exit 1
fi

# Remove the taint from the node
remove_taint() {
    echo "Removing taint $TAINT_KEY from node $NODE_NAME"

    if kubectl taint nodes "$NODE_NAME" "$TAINT_KEY-"; then
        echo "Taint removed successfully"
    else
        if kubectl get node "$NODE_NAME" -o jsonpath="{.spec.taints[*].key}" | grep -q "$TAINT_KEY"; then
            echo "Error: Failed to remove taint and it still exists on the node"
            exit 1
        else
            echo "Note: Taint was already removed from the node"
        fi
    fi
}

# Main execution
ACTION=${1:-"remove"}

case "$ACTION" in
    remove)
        remove_taint
        ;;
    *)
        echo "Usage: $0 [remove]"
        exit 1
        ;;
esac

exit 0
