PROJECT=simkube
DOCKER_REGISTRY=localhost:5000
KIND_CLUSTER_NAME=test

MANIFESTS:=$(wildcard manifests/*.yml)

.PHONY: build lint test cover image run clean kind

build:
	CGO_ENABLED=0 go build -trimpath -o output/${PROJECT} main.go

lint:
	golangci-lint run

test:
	go test ./...

cover:
	go-carpet | less -R

image:
	docker build output -f images/Dockerfile -t ${DOCKER_REGISTRY}/${PROJECT}:latest
	docker push ${DOCKER_REGISTRY}/${PROJECT}:latest

run: .applied-simkube
	kubectl rollout restart deployment ${PROJECT}

kind: .applied-kind .applied-prometheus

clean:
	rm -rf .applied-*
	rm -rf kind/kube-prometheus/manifests
	kind delete cluster --name ${KIND_CLUSTER_NAME}

.applied-simkube: ${MANIFESTS}
	@echo $? | xargs -d' ' -L1 kubectl apply -f
	@touch $@

.applied-kind: kind/kind-config.yml kind/local-registry-hosting.yml kind/certs.sh
	kind delete cluster --name ${KIND_CLUSTER_NAME}
	kind create cluster --name ${KIND_CLUSTER_NAME} --config=kind/kind-config.yml
	kind/certs.sh
	kubectl apply -f kind/local-registry-hosting.yml
	touch $@

.applied-prometheus: kind/kube-prometheus/simkube.jsonnet
	cd kind/kube-prometheus && ./build.sh simkube.jsonnet
	kubectl apply --server-side -f kind/kube-prometheus/manifests/setup
	kubectl wait --for condition=Established --all CustomResourceDefinition --namespace=monitoring
	kubectl apply -f kind/kube-prometheus/manifests
	touch $@
