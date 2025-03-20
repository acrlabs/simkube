ARTIFACTS=sk-ctrl sk-driver sk-tracer

include build/base.mk
include build/rust.mk
include build/image.mk
include build/k8s.mk

RUST_BUILD_IMAGE ?= rust:1.79-bullseye
CARGO=CARGO_HOME=$(BUILD_DIR)/cargo cargo
COVERAGE_IGNORES+='sk-api/*' '*/testutils/*'

ifndef IN_CI
DOCKER_ARGS=-it --init
endif

# deliberately override the basic rust rule
build: main skctl
	@true

.PHONY: build-docker
build-docker: _version
	$(CARGO) build $(addprefix -p=,$(ARTIFACTS)) --color=always
	cp $(addprefix $(BUILD_DIR)/debug/,$(ARTIFACTS)) $(BUILD_DIR)/.

.PHONY: main
main:
	docker run $(DOCKER_ARGS) -u `id -u`:`id -g` -w /build -v `pwd`:/build:rw $(RUST_BUILD_IMAGE) make build-docker

# This is sorta subtle; the three "main" artifacts get built inside docker containers
# to ensure that they are built against the right libs that they'll be running on in
# the cluster.  So for those we share CARGO_HOME, which needs to be in $(BUILD_DIR)
# so we have a known location for it.  This is _not_ built in a docker container because
# it's designed to run on the user's machine, so we don't use the custom CARGO_HOME
skctl:
	cargo build --profile skctl-dev -p=skctl --color=always
	cp $(BUILD_DIR)/skctl-dev/skctl $(BUILD_DIR)/.


IMAGE_DEPS += metrics_config

.PHONY: metrics_config
metrics_config:
	cp -r examples/metrics $(BUILD_DIR)/metrics-cfg

K8S_DEPS += crd

.PHONY: crd
crd: skctl
	$(BUILD_DIR)/skctl crd > k8s/raw/simkube.io_simulations.yml

.PHONY: validation_rules
validation_rules: VALIDATION_FILE=sk-cli/src/validation/rules/README.md
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
