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

skctl:
	CGO_ENABLED=0 go build -trimpath -o $(BUILD_DIR)/skctl ./cli/

$(GO_ARTIFACTS):
	CGO_ENABLED=0 go build -trimpath -o $(BUILD_DIR)/$@ ./$(subst sk-,,$(@))/cmd/

RUST_BUILD_IMAGE ?= rust:buster

$(RUST_ARTIFACTS):
	mkdir -p .build
	docker run -u `id -u`:`id -g` -w /build -v `pwd`:/build:ro -v $(BUILD_DIR):/build/.build:rw $(RUST_BUILD_IMAGE) make $@-docker

%-docker:
	$(CARGO_HOME_ENV) cargo build --target-dir=$(BUILD_DIR) --bin=$* --color=always
	cp $(BUILD_DIR)/debug/$* $(BUILD_DIR)/.

lint:
	$(CARGO_HOME_ENV) cargo clippy
	golangci-lint run

test: test-go test-rust itest-rust

.PHONY: test-go
test-go:
	mkdir -p $(BUILD_DIR)/coverage
	go test -coverprofile=$(GO_COVER_FILE) ./...

.PHONY: test-rust
test-rust:
	mkdir -p $(BUILD_DIR)/coverage
	rm -f $(BUILD_DIR)/coverage/*.profraw
	$(CARGO_TEST_PREFIX) cargo test --features=testutils $(CARGO_TEST) $(patsubst %, --bin %, $(RUST_ARTIFACTS)) --lib -- --nocapture --skip itest

.PHONY: itest-rust
itest-rust:
	$(CARGO_TEST_PREFIX) cargo test --features=testutils itest --lib -- --nocapture

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
		--ignore '*/rust/api/v1/*' \
		--ignore '.build/cargo/*' \
		--ignore 'hack/*' \
		--excl-line '#\[derive' \
		--excl-start '#\[cfg\((test|feature = "testutils")'
	@if [ "$(RUST_COVER_TYPE)" = "markdown" ]; then cat $(RUST_COVER_FILE); fi

.PHONY: crd
crd:
	controller-gen crd object paths=./lib/go/api/v1/... output:artifacts:config=./k8s/raw
	kopium -f k8s/raw/simkube.io_simulationroots.yaml > lib/rust/api/v1/simulation_roots.rs
	kopium -f k8s/raw/simkube.io_simulations.yaml > lib/rust/api/v1/simulations.rs

.PHONY: api
api:
	openapi-generator generate -i api/v1/simkube.yml -g go -o generated-api
	openapi-generator generate -i api/v1/simkube.yml -g rust --global-property models -o generated-api
	cp generated-api/model_export_filters.go lib/go/api/v1/export_filters.go
	cp generated-api/model_export_request.go lib/go/api/v1/export_request.go
	cp generated-api/utils.go lib/go/api/v1/utils.go
	cp generated-api/src/models/export_filters.rs lib/rust/api/v1/.
	cp generated-api/src/models/export_request.rs lib/rust/api/v1/.
	@echo ''
	@echo '----------------------------------------------------------------------'
	@echo 'WARNING: YOU NEED TO DO MANUAL CLEANUP TO THE OPENAPI GENERATED FILES!'
	@echo '----------------------------------------------------------------------'
	@echo 'At a minimum:'
	@echo '1. In lib/go/api/v1/*, replace all the k8s-generated types with the'
	@echo '   correct imports from the k8s api'
	@echo '2. In lib/go/api/v1/*, change the package name from "openapi" to "v1"'
	@echo '3. In lib/go/api/v1/*, make sure you annotate the generated objects'
	@echo '   with "//+kubebuilder:object:generate=false" so that controller-gen'
	@echo '   does not get confused.'
	@echo '4. In lib/rust/api/v1/*, add "use super::*", and replace all the'
	@echo '   k8s-generated types with the correct imports from k8s-openapi'
	@echo '----------------------------------------------------------------------'
	@echo 'CHECK THE DIFF CAREFULLY!!!'
	@echo '----------------------------------------------------------------------'
	@echo ''
	@echo 'Eventually we would like to automate more of this, but it does not'
	@echo 'happen right now.  :('
	@echo ''
