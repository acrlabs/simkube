#!/usr/bin/env bash
set -euo pipefail

if [[ ! -f .build/sk-driver-image ]]; then
    echo "❌ Missing .build/sk-driver-image"
    exit 1
fi

IMAGE=$(cat .build/sk-driver-image)

if [[ "$IMAGE" != localhost:5000/* ]]; then
    echo "❌ Driver image is not from local registry: $IMAGE"
    exit 1
fi

echo "Driver image: $IMAGE"
echo "driver_image=$IMAGE" >> "$GITHUB_OUTPUT"
