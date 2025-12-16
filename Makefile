ARTIFACTS=sk-ctrl sk-driver sk-tracer
DISPATCH_MODE=recurse
BUILD_TARGETS=main skctl

include build/base.mk
include build/rust.mk
include build/image.mk
include build/k8s.mk

RUST_BUILD_IMAGE ?= rust:1.88-bookworm  # if this changes make sure to update the Dockerfiles to match
COVERAGE_IGNORES+=sk-api/.* testutils/.*
EXCLUDE_CRATES=sk-testutils
RUST_LOG=warn,sk_api,sk_core,sk_store,sk_tracer,sk_ctrl,sk_driver,sk_cli,httpmock=debug

ifndef IN_CI
# Make ctrl-C work in the middle of a build
DOCKER_ARGS=-it --init
endif

.PHONY: main
main:
	docker run $(DOCKER_ARGS) -u `id -u`:`id -g` -w /build -v `pwd`:/build:rw $(RUST_BUILD_IMAGE) \
		scripts/build-in-docker "$(BUILD_DIR)" "$(BUILD_MODE)" "$(ARTIFACTS)"

.PHONY: skctl
skctl:
	make build DISPATCH_MODE=local ARTIFACTS=skctl

IMAGE_DEPS += copy-config

.PHONY: copy-config
copy-config:
	rm -rf $(BUILD_DIR)/config
	cp -r config $(BUILD_DIR)/config

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
	@printf '\n'
	@printf '----------------------------------------------------------------------\n'
	@printf 'WARNING: YOU NEED TO DO MANUAL CLEANUP TO THE OPENAPI GENERATED FILES!\n'
	@printf '----------------------------------------------------------------------\n'
	@printf 'At a minimum:\n'
	@printf '   In sk-api/src/v1/*, add "use super::*", and replace all the\n'
	@printf '   k8s-generated types with the correct imports from k8s-openapi\n'
	@printf '----------------------------------------------------------------------\n'
	@printf 'CHECK THE DIFF CAREFULLY!!!\n'
	@printf '----------------------------------------------------------------------\n'
	@printf '\n'
	@printf 'Eventually we would like to automate more of this, but it does not\n'
	@printf 'happen right now.  :(\n'
	@printf '\n'
