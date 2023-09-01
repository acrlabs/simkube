GO_ARTIFACTS=sk-cloudprov sk-vnode
RUST_ARTIFACTS=sk-ctrl sk-driver sk-tracer
ARTIFACTS ?= $(GO_ARTIFACTS) $(RUST_ARTIFACTS)

COVERAGE_DIR=$(BUILD_DIR)/coverage
GO_COVER_FILE=$(COVERAGE_DIR)/go-coverage.txt

include build/base.mk

setup::
	cargo vendor .vendor

$(GO_ARTIFACTS):
	CGO_ENABLED=0 go build -trimpath -o $(BUILD_DIR)/$@ ./$(subst sk-,,$(@))/cmd/

RUST_BUILD_IMAGE ?= rust:buster

$(RUST_ARTIFACTS):
	docker run -u `id -u`:`id -g` -w /build -v `pwd`:/build:ro -v $(BUILD_DIR):/build/.build:rw $(RUST_BUILD_IMAGE) make $@-docker

%-docker:
	cargo build --target-dir=$(BUILD_DIR) --bin=$* --color=always
	cp $(BUILD_DIR)/debug/$* $(BUILD_DIR)/.

lint:
	cargo clippy
	golangci-lint run

test:
	mkdir -p $(BUILD_DIR)/coverage
	go test -coverprofile=$(GO_COVER_FILE) ./...
	CARGO_INCREMENTAL=0 RUSTFLAGS='-Cinstrument-coverage' LLVM_PROFILE_FILE='$(BUILD_DIR)/cargo-test-%p-%m.profraw' cargo test --target-dir=$(BUILD_DIR)/test
	grcov . --binary-path $(BUILD_DIR)/test/debug/deps -s . -t lcov,markdown --branch --ignore '../*' --ignore '/*' --ignore '.vendor/*' --ignore 'tests/*' -o $(BUILD_DIR)/coverage

cover:
	go tool cover -func=$(GO_COVER_FILE)
	cat $(COVERAGE_DIR)/markdown.md

clean::
	rm -rf .vendor
