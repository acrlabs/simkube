DOCKER_REGISTRY ?= localhost:5000

IMAGE_DEPS =
IMAGE_TARGETS = $(addprefix images/$(BUILD_MODE)/Dockerfile.,$(ARTIFACTS))
IMAGE_TAG ?=
_DEFAULT_BUILD_TARGETS += image

.PHONY: _image
_image::
	$(if $(IMAGE_DEPS),$(MAKE) $(IMAGE_DEPS),,)
	IMAGE_TAG="$(IMAGE_TAG)" && \
		if [ -z "$$IMAGE_TAG" ]; then IMAGE_TAG="$$(scripts/image-tag)"; fi && \
		$(MAKE) $(IMAGE_TARGETS) IMAGE_TAG="$$IMAGE_TAG"

.PHONY: $(IMAGE_TARGETS)
$(IMAGE_TARGETS):
	PROJECT_NAME=$(subst images/$(BUILD_MODE)/Dockerfile.,,$@) && \
		IMAGE_NAME=$(DOCKER_REGISTRY)/$$PROJECT_NAME:$(IMAGE_TAG) && \
		docker build $(BUILD_DIR) -f $@ -t $$IMAGE_NAME && \
		docker push $$IMAGE_NAME && \
		printf "%s" "$$IMAGE_NAME" > $(BUILD_DIR)/$${PROJECT_NAME}-image

.PHONY: image
image: _image
