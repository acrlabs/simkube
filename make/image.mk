DOCKER_REGISTRY ?= localhost:5000
CONTAINER_ENGINE ?= docker

IMAGE_DEPS =
IMAGE_TARGETS = $(addprefix images/$(BUILD_MODE)/Dockerfile.,$(ARTIFACTS))
IMAGE_TAG ?=
_DEFAULT_BUILD_TARGETS += image

.PHONY: _image
_image::
	$(if $(IMAGE_DEPS),$(MAKE) $(IMAGE_DEPS),,)
	rm -f $(addsuffix -image,$(addprefix $(BUILD_DIR)/,$(ARTIFACTS)))
	printf '%s\n' $(ARTIFACTS) > $(BUILD_DIR)/image-artifacts
	IMAGE_TAG="$(IMAGE_TAG)" && \
		if [ -z "$$IMAGE_TAG" ]; then IMAGE_TAG="$$(scripts/image-tag)"; fi && \
		$(MAKE) $(IMAGE_TARGETS) IMAGE_TAG="$$IMAGE_TAG"

.PHONY: $(IMAGE_TARGETS)
$(IMAGE_TARGETS):
	PROJECT_NAME=$(subst images/$(BUILD_MODE)/Dockerfile.,,$@) && \
		IMAGE_NAME=$(DOCKER_REGISTRY)/$$PROJECT_NAME:$(IMAGE_TAG) && \
		$(CONTAINER_ENGINE) build --platform $(TARGET_PLATFORM) \
			--build-arg TARGETARCH=$(TARGET_ARCH) -f $@ -t $$IMAGE_NAME $(BUILD_DIR) && \
		printf "%s" "$$IMAGE_NAME" > $(BUILD_DIR)/$${PROJECT_NAME}-image

.PHONY: image
image: _image
