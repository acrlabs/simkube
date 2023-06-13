PROJECT=simkube

.PHONY: build test

build:
	go build -o ${PROJECT} main.go

test:
	go test
