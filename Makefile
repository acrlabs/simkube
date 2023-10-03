GO_ARTIFACTS=sk-cloudprov sk-vnode
RUST_ARTIFACTS=sk-ctrl sk-driver sk-tracer
ARTIFACTS ?= $(GO_ARTIFACTS) $(RUST_ARTIFACTS)

COVERAGE_DIR=$(BUILD_DIR)/coverage
GO_COVER_FILE=$(COVERAGE_DIR)/go-coverage.txt
CARGO_HOME_ENV=CARGO_HOME=$(BUILD_DIR)/cargo

ifdef WITH_COVERAGE
CARGO_TEST_PREFIX=$(CARGO_HOME_ENV) CARGO_INCREMENTAL=0 RUSTFLAGS='-Cinstrument-coverage' LLVM_PROFILE_FILE='$(BUILD_DIR)/coverage/cargo-test-%p-%m.profraw'
RUST_COVER_TYPE ?= lcov
else
CARGO_TEST_PREFIX=$(CARGO_HOME_ENV)
RUST_COVER_TYPE=markdown
endif

RUST_COVER_FILE=$(COVERAGE_DIR)/rust-coverage.$(RUST_COVER_TYPE)

include build/base.mk

$(GO_ARTIFACTS):
	CGO_ENABLED=0 go build -trimpath -o $(BUILD_DIR)/$@ ./$(subst sk-,,$(@))/cmd/

RUST_BUILD_IMAGE ?= rust:buster

$(RUST_ARTIFACTS):
	docker run -u `id -u`:`id -g` -w /build -v `pwd`:/build:ro -v $(BUILD_DIR):/build/.build:rw $(RUST_BUILD_IMAGE) make $@-docker

%-docker:
	$(CARGO_HOME_ENV) cargo build --target-dir=$(BUILD_DIR) --bin=$* --color=always
	cp $(BUILD_DIR)/debug/$* $(BUILD_DIR)/.

lint:
	$(CARGO_HOME_ENV) cargo clippy
	golangci-lint run

test: test-go test-rust

.PHONY: test-go
test-go:
	mkdir -p $(BUILD_DIR)/coverage
	go test -coverprofile=$(GO_COVER_FILE) ./...

.PHONY: test-rust
test-rust:
	mkdir -p $(BUILD_DIR)/coverage
	rm -f $(BUILD_DIR)/coverage/*.profraw
	$(CARGO_TEST_PREFIX) cargo test $(CARGO_TEST) -- --nocapture

cover: cover-go cover-rust

.PHONY: cover-go
cover-go:
	go tool cover -func=$(GO_COVER_FILE)

.PHONY: cover-rust
cover-rust:
	grcov . --binary-path $(BUILD_DIR)/debug/deps -s . -t $(RUST_COVER_TYPE) -o $(RUST_COVER_FILE) --branch \
		--ignore '../*' \
		--ignore '/*' \
		--ignore '*/tests/*' \
		--ignore '*_test.rs' \
		--ignore '*/testutils/*' \
		--ignore '.build/cargo/*' \
		--ignore 'hack/*' \
		--excl-line '#\[derive' \
		--excl-start '#\[cfg\(test'
	@if [ "$(RUST_COVER_TYPE)" = "markdown" ]; then cat $(RUST_COVER_FILE); fi

.PHONY: crd
crd:
	cargo run --bin gencrd > $(BUILD_DIR)/manifests/0000-crd.yml
