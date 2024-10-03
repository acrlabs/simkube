ARTIFACTS ?= sk-ctrl sk-driver sk-tracer

COVERAGE_DIR=$(BUILD_DIR)/coverage
CARGO_HOME_ENV=CARGO_HOME=$(BUILD_DIR)/cargo

ifdef IN_CI
CARGO_TEST_PREFIX=$(CARGO_HOME_ENV) CARGO_INCREMENTAL=0 RUSTFLAGS='-Cinstrument-coverage' LLVM_PROFILE_FILE='$(BUILD_DIR)/coverage/cargo-test-%p-%m.profraw'
RUST_COVER_TYPE ?= lcov
DOCKER_ARGS=
else
RUST_COVER_TYPE=markdown
DOCKER_ARGS=-it --init
endif

RUST_COVER_FILE=$(COVERAGE_DIR)/rust-coverage.$(RUST_COVER_TYPE)
APP_VERSION_CMD=tomlq -r .workspace.package.version Cargo.toml
APP_VERSION=$(shell $(APP_VERSION_CMD))

include build/base.mk
include build/k8s.mk

RUST_BUILD_IMAGE ?= rust:1.79-bullseye

main:
	docker run $(DOCKER_ARGS) -u `id -u`:`id -g` -w /build -v `pwd`:/build:rw -v $(BUILD_DIR):/build/.build:rw $(RUST_BUILD_IMAGE) make build-docker

extra: skctl

# This is sorta subtle; the three "main" artifacts get built inside docker containers
# to ensure that they are built against the right libs that they'll be running on in
# the cluster.  So for those we share CARGO_HOME_ENV, which needs to be in $(BUILD_DIR)
# so we have a known location for it.  This is _not_ built in a docker container so that
# because it's designed to run on the user's machine, so we don't use the custom CARGO_HOME_ENV
skctl:
	cargo build --target-dir=$(BUILD_DIR) -p=skctl --color=always
	cp $(BUILD_DIR)/debug/skctl $(BUILD_DIR)/.

pre-image:
	cp -r examples/metrics $(BUILD_DIR)/metrics-cfg

build-docker:
	$(CARGO_HOME_ENV) cargo build --target-dir=$(BUILD_DIR) $(addprefix -p=,$(ARTIFACTS)) --color=always
	cp $(addprefix $(BUILD_DIR)/debug/,$(ARTIFACTS)) $(BUILD_DIR)/.

test: unit itest

.PHONY: unit
unit:
	mkdir -p $(BUILD_DIR)/coverage
	rm -f $(BUILD_DIR)/coverage/*.profraw
	$(CARGO_TEST_PREFIX) cargo test $(CARGO_TEST) --features testutils -- --skip itest

.PHONY: itest
itest:
	$(CARGO_TEST_PREFIX) cargo test itest --features testutils -- --nocapture --test-threads=1

lint:
	pre-commit run --all

cover:
	grcov . --binary-path $(BUILD_DIR)/debug/deps -s . -t $(RUST_COVER_TYPE) -o $(RUST_COVER_FILE) --branch \
		--ignore '../*' \
		--ignore '/*' \
		--ignore '*/tests/*' \
		--ignore '*_test.rs' \
		--ignore 'sk-api/*' \
		--ignore 'testutils/*' \
		--ignore '.build/*' \
		--excl-line '#\[derive' \
		--excl-start '#\[cfg\((test|feature = "testutils")'
	@if [ "$(RUST_COVER_TYPE)" = "markdown" ]; then cat $(RUST_COVER_FILE); fi

.PHONY: release-patch release-minor release-major
release-patch release-minor release-major:
	cargo set-version --bump $(subst release-,,$@)
	make kustomize
	NEW_APP_VERSION=`$(APP_VERSION_CMD)` && \
		git commit -a -m "release: version v$$NEW_APP_VERSION" && \
		git tag v$$NEW_APP_VERSION
	cargo ws publish --publish-as-is

.PHONY: crd
crd: skctl
	$(BUILD_DIR)/skctl crd > k8s/raw/simkube.io_simulations.yml

pre-k8s:: crd

.PHONY: validation_rules
validation_rules: VALIDATION_FILE=sk-cli/src/validation/README.md
validation_rules: skctl
	printf "# SimKube Trace Validation Checks\n\n" > $(VALIDATION_FILE)
	$(BUILD_DIR)/skctl validate print --format table >> $(VALIDATION_FILE)
	printf "\nThis file is auto-generated; to rebuild, run \`make $@\`.\n" >> $(VALIDATION_FILE)

.PHONY: api
api:
	openapi-generator generate -i sk-api/schema/v1/simkube.yml -g rust --global-property models -o generated-api
	cp generated-api/src/models/export_filters.rs sk-api/src/v1/.
	cp generated-api/src/models/export_request.rs sk-api/src/v1/.
	@echo ''
	@echo '----------------------------------------------------------------------'
	@echo 'WARNING: YOU NEED TO DO MANUAL CLEANUP TO THE OPENAPI GENERATED FILES!'
	@echo '----------------------------------------------------------------------'
	@echo 'At a minimum:'
	@echo '   In sk-api/src/v1/*, add "use super::*", and replace all the'
	@echo '   k8s-generated types with the correct imports from k8s-openapi'
	@echo '----------------------------------------------------------------------'
	@echo 'CHECK THE DIFF CAREFULLY!!!'
	@echo '----------------------------------------------------------------------'
	@echo ''
	@echo 'Eventually we would like to automate more of this, but it does not'
	@echo 'happen right now.  :('
	@echo ''
