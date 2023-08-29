GO_ARTIFACTS=sk-cloudprov sk-vnode
RUST_ARTIFACTS=sk-ctrl sk-driver sk-tracer
ARTIFACTS=$(GO_ARTIFACTS) $(RUST_ARTIFACTS)

include build/base.mk

setup::
	cargo vendor .vendor

$(GO_ARTIFACTS):
	CGO_ENABLED=0 go build -trimpath -o $(BUILD_DIR)/$@ ./$(subst sk-,,$(@))/cmd/

RUST_BUILD_IMAGE ?= rust:buster

$(RUST_ARTIFACTS):
	docker run -u `id -u`:`id -g` -w /build -v `pwd`:/build:ro -v $(BUILD_DIR):/build/.build:rw $(RUST_BUILD_IMAGE) make $@-docker

%-docker:
	cargo build --target-dir=$(BUILD_DIR) --bin=$*
	cp $(BUILD_DIR)/debug/$* $(BUILD_DIR)/.

lint:
	golangci-lint run

test:
	go test ./...
	cargo test

cover:
	go-carpet -summary
