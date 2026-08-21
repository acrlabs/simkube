set -euo pipefail

PODS=(
    sk-ctrl
    sk-tracer
)

for POD in "${PODS[@]}"; do
    echo "Inspecting pod: $POD..."
    POD_NAME=$(kubectl get pod \
        --field-selector=status.phase=Running \
        -l app.kubernetes.io/name="$POD" \
        -n simkube \
        -o jsonpath='{.items[0].metadata.name}')

    IMAGE_ID=$(kubectl get pod "$POD_NAME" \
        -n simkube \
        -o jsonpath='{.status.containerStatuses[0].imageID}')

    echo "Pod: $POD_NAME"
    echo "Image ID: $IMAGE_ID"

    if ! echo "$IMAGE_ID" | grep -q "localhost:5000"; then
        echo "❌ Image NOT from local registry"
        exit 1
    fi

    echo "✅ $POD image is from local registry"
done
