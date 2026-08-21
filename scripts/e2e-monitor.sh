#!/usr/bin/env bash
set -euo pipefail

while true; do
    printf '=== %s ===\n' "$(date -Is)"
    kubectl get pods -A | awk 'NR==1 || $1 ~ /virtual-/' 2>&1
    sleep 30
done
