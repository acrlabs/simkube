ARTIFACTS=sk-ctrl sk-driver sk-tracer
EXTRA_BUILD_ARTIFACTS=skctl

COVERAGE_DIR=$(BUILD_DIR)/coverage
CARGO_HOME_ENV=CARGO_HOME=$(BUILD_DIR)/cargo

ifdef IN_CI
CARGO_TEST_PREFIX=$(CARGO_HOME_ENV) CARGO_INCREMENTAL=0 RUSTFLAGS='-Cinstrument-coverage' LLVM_PROFILE_FILE='$(BUILD_DIR)/coverage/cargo-test-%p-%m.profraw'
RUST_COVER_TYPE ?= lcov
DOCKER_ARGS=
else
CARGO_TEST_PREFIX=$(CARGO_HOME_ENV)
RUST_COVER_TYPE=markdown
DOCKER_ARGS=-it
endif

RUST_COVER_FILE=$(COVERAGE_DIR)/rust-coverage.$(RUST_COVER_TYPE)

include build/base.mk
include build/k8s.mk

RUST_BUILD_IMAGE ?= rust:buster

$(EXTRA_BUILD_ARTIFACTS)::
	$(CARGO_HOME_ENV) cargo build --target-dir=$(BUILD_DIR) --bin=$@ --color=always
	cp $(BUILD_DIR)/debug/$@ $(BUILD_DIR)/.

$(ARTIFACTS)::
	docker run $(DOCKER_ARGS) -u `id -u`:`id -g` -w /build -v `pwd`:/build:ro -v $(BUILD_DIR):/build/.build:rw $(RUST_BUILD_IMAGE) make $@-docker

%-docker:
	$(CARGO_HOME_ENV) cargo build --target-dir=$(BUILD_DIR) --bin=$* --color=always
	cp $(BUILD_DIR)/debug/$* $(BUILD_DIR)/.

lint:
	$(CARGO_HOME_ENV) cargo +nightly fmt
	$(CARGO_HOME_ENV) cargo clippy

test: test-unit test-int

.PHONY: test-unit
test-unit:
	mkdir -p $(BUILD_DIR)/coverage
	rm -f $(BUILD_DIR)/coverage/*.profraw
	$(CARGO_TEST_PREFIX) cargo test --features=testutils $(CARGO_TEST) $(patsubst %, --bin %, $(RUST_ARTIFACTS)) --lib -- --nocapture --skip itest

.PHONY: test-int
test-int:
	$(CARGO_TEST_PREFIX) cargo test --features=testutils itest --lib -- --nocapture

cover:
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
crd: skctl
	$(BUILD_DIR)/skctl crd > k8s/raw/simkube.io_simulations.yml

.PHONY: api
api:
	openapi-generator generate -i api/v1/simkube.yml -g rust --global-property models -o generated-api
	cp generated-api/src/models/export_filters.rs lib/rust/api/v1/.
	cp generated-api/src/models/export_request.rs lib/rust/api/v1/.
	@echo ''
	@echo '----------------------------------------------------------------------'
	@echo 'WARNING: YOU NEED TO DO MANUAL CLEANUP TO THE OPENAPI GENERATED FILES!'
	@echo '----------------------------------------------------------------------'
	@echo 'At a minimum:'
	@echo '   In lib/rust/api/v1/*, add "use super::*", and replace all the'
	@echo '   k8s-generated types with the correct imports from k8s-openapi'
	@echo '----------------------------------------------------------------------'
	@echo 'CHECK THE DIFF CAREFULLY!!!'
	@echo '----------------------------------------------------------------------'
	@echo ''
	@echo 'Eventually we would like to automate more of this, but it does not'
	@echo 'happen right now.  :('
	@echo ''
