#!/bin/bash
set -e
# Assert dependencies
assert_command() {
    if ! command -v "$1" &> /dev/null; then
        echo "Error: $1 is not installed or not in PATH. Please install $1 and try again."
        exit 1
    fi
}
assert_command git
assert_command kind
assert_command kubectl
assert_command helm
assert_command curl
assert_command jq
assert_command cargo
# Install skctl if not already installed
if ! command -v skctl &> /dev/null; then
    cargo install skctl || {
        echo "Failed to install skctl"
        exit 1
    }
fi
TRACE_PATH="${TRACE_PATH:-file:///$HOME/data/trace}"
PROD_CONTEXT=kind-dsb
SIM_CONTEXT=kind-simkube
# Clone repositories
clone_repo() {
    local repo_url="$1"
    local dir_name="$2"
    local branch="$3"
    if [ ! -d "$dir_name" ]; then
        echo "Cloning $dir_name..."
        git clone "$repo_url" "$dir_name"
        if [ -n "$branch" ]; then
            (cd "$dir_name" && git checkout "$branch")
        fi
    else
        echo "$dir_name already exists, skipping clone."
    fi
}
clone_repo "https://github.com/prometheus-operator/kube-prometheus.git" "kube-prometheus"

# make simkube cluster
kind create cluster --config - <<EOF
kind: Cluster
name: simkube
apiVersion: kind.x-k8s.io/v1alpha4
nodes:
  - role: control-plane
    labels:
      type: kind-control-plane
  - role: worker
    labels:
      type: kind-worker 
  - role: worker
    extraMounts:
      - hostPath: /tmp/kind-node-data
        containerPath: /data
EOF
# KWOK
KWOK_REPO=kubernetes-sigs/kwok
KWOK_LATEST_RELEASE=$(curl "https://api.github.com/repos/${KWOK_REPO}/releases/latest" | jq -r '.tag_name')
kubectl apply --context=${SIM_CONTEXT} -f "https://github.com/${KWOK_REPO}/releases/download/${KWOK_LATEST_RELEASE}/kwok.yaml"
kubectl apply --context=${SIM_CONTEXT} -f "https://github.com/${KWOK_REPO}/releases/download/${KWOK_LATEST_RELEASE}/stage-fast.yaml"
# Prometheus
cd kube-prometheus
kubectl --context=${SIM_CONTEXT} create -f manifests/setup
until kubectl --context=${SIM_CONTEXT} get servicemonitors --all-namespaces ; do date; sleep 1; echo ""; done
kubectl --context=${SIM_CONTEXT} create -f manifests/
cd ..
# Cert Manager
kubectl --context=${SIM_CONTEXT} apply -f https://github.com/cert-manager/cert-manager/releases/download/v1.14.3/cert-manager.yaml
kubectl --context=${SIM_CONTEXT} wait --for=condition=Ready -l app=webhook -n cert-manager pod --timeout=180s
kubectl --context=${SIM_CONTEXT} apply -f - <<EOF
apiVersion: cert-manager.io/v1
kind: ClusterIssuer
metadata:
  name: selfsigned
  namespace: kube-system
spec:
  selfSigned: {}
EOF
# add fake nodes
kubectl --context=${SIM_CONTEXT} apply -f - <<EOF
apiVersion: v1
kind: Node
metadata:
  annotations:
    kwok.x-k8s.io/node: fake
  labels:
    node.kubernetes.io/instance-type: c5d.9xlarge
    topology.kubernetes.io/zone: us-west-1a
    type: virtual
  name: fake-node-1
spec:
  taints:
  - effect: NoSchedule
    key: kwok-provider
    value: "true"
status:
  allocatable:
    cpu: 35
    ephemeral-storage: 900Gi
    memory: 71Gi
    pods: 110
  capacity:
    cpu: 36
    ephemeral-storage: 900Gi
    memory: 72Gi
    pods: 110
EOF
# Apply SimKube configurations
kubectl --context ${SIM_CONTEXT} apply -k k8s/kustomize/sim

