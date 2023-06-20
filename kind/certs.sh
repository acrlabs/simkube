REG_PORT=5000
CLUSTER_NAME=test
REGISTRY_DIR="/etc/containerd/certs.d/localhost:${REG_PORT}"

for node in $(kind get nodes --name ${CLUSTER_NAME}); do
  docker exec "${node}" mkdir -p "${REGISTRY_DIR}"
  cat <<EOF | docker exec -i "${node}" cp /dev/stdin "${REGISTRY_DIR}/hosts.toml"
[host."http://kind-registry:${REG_PORT}"]
EOF
done
