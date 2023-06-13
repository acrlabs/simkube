PROJECT=simkube

.PHONY: build test image

build:
	CGO_ENABLED=0 go build -trimpath -o output/${PROJECT} main.go

test:
	go test

image:
	docker build output -f images/Dockerfile -t localhost:5000/${PROJECT}:latest
	docker push localhost:5000/${PROJECT}:latest

run:
	kubectl rollout restart deployment ${PROJECT} || kubectl apply -f manifests/deployment.yml
