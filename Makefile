PROJECTS=simkube sk-cloudprov
IMAGE_TARGETS=$(addprefix images/Dockerfile.,$(PROJECTS))
PROJECT_RUNNERS=$(addprefix run-,$(PROJECTS))
DOCKER_REGISTRY=localhost:5000
KIND_CLUSTER_NAME=test

MANIFESTS:=$(wildcard manifests/*.yml)
KIND_CONFIGS:=$(wildcard kind/*.yml)

.PHONY: build image run $(PROJECTS) $(IMAGE_TARGETS) $(PROJECT_RUNNERS) lint test cover kind clean

build: $(PROJECTS)

image: $(IMAGE_TARGETS)

run: $(PROJECT_RUNNERS)

$(PROJECTS):
	CGO_ENABLED=0 go build -trimpath -o output/$@ ./cmd/$@

lint:
	golangci-lint run

test:
	go test ./...

cover:
	go-carpet | less -R

$(IMAGE_TARGETS):
	docker build output -f $@ -t $(DOCKER_REGISTRY)/$(subst images/Dockerfile.,,$@):latest
	docker push $(DOCKER_REGISTRY)/$(subst images/Dockerfile.,,$@):latest

$(PROJECT_RUNNERS): .applied-simkube
	kubectl rollout restart deployment $(subst run-,,$@)

kind: .applied-kind # .applied-prometheus

clean:
	rm -rf output
	rm -rf .applied-*
	rm -rf kind/kube-prometheus/manifests
	kind delete cluster --name $(KIND_CLUSTER_NAME)

.applied-simkube: $(MANIFESTS)
	@echo $? | xargs -d' ' -L1 kubectl apply -f
	@touch $@

.applied-kind: kind/certs.sh $(KIND_CONFIGS)
	kind delete cluster --name $(KIND_CLUSTER_NAME)
	kind create cluster --name $(KIND_CLUSTER_NAME) --config=kind/kind-config.yml
	kind/certs.sh
	kubectl apply -f kind/local-registry-hosting.yml
	kubectl patch -n kube-system ds kindnet --patch-file kind/kindnet-patch.yml
	kubectl apply -f kind/cluster-autoscaler.yml
	touch $@

.applied-prometheus: kind/kube-prometheus/simkube.jsonnet kind/kube-prometheus/node-exporter-patch.yml
	cd kind/kube-prometheus && ./build.sh simkube.jsonnet
	kubectl apply --server-side -f kind/kube-prometheus/manifests/setup
	kubectl wait --for condition=Established --all CustomResourceDefinition --namespace=monitoring
	kubectl apply -f kind/kube-prometheus/manifests
	kubectl patch -n monitoring ds node-exporter --patch-file kind/kube-prometheus/node-exporter-patch.yml
	touch $@
