BUILD_DIR=$(shell pwd)/.build
PROJECTS=sk-vnode sk-cloudprov
IMAGE_TARGETS=$(addprefix images/Dockerfile.,$(PROJECTS))
DOCKER_REGISTRY=localhost:5000
SHA=$(shell git rev-parse --short HEAD)
UNCLEAN_TREE_SUFFIX=-$(shell test -z "$(git status --porcelain --untracked-files=no)" || \
	GIT_INDEX_FILE=`mktemp` git add -u && git write-tree && git reset -q && rm $$GIT_INDEX_FILE)

.PHONY: setup default test build image run $(PROJECTS) $(IMAGE_TARGETS) lint cover clean

default: build image run

test: lint cover

setup:
	pre-commit install
	cd k8s && poetry install

build: $(PROJECTS)

image: $(IMAGE_TARGETS)

run:
	export CDK8S_OUTDIR=$(BUILD_DIR)/manifests && export BUILD_DIR=$(BUILD_DIR) && cd k8s && poetry run ./main.py
	kubectl apply -f $(BUILD_DIR)/manifests

$(PROJECTS):
	CGO_ENABLED=0 go build -trimpath -o $(BUILD_DIR)/$@ ./cmd/$@

lint:
	golangci-lint run

cover:
	go-carpet -summary

$(IMAGE_TARGETS):
	PROJECT_NAME=$(subst images/Dockerfile.,,$@) && \
		IMAGE_NAME=$(DOCKER_REGISTRY)/$$PROJECT_NAME:$(SHA)$(UNCLEAN_TREE_SUFFIX) && \
		docker build $(BUILD_DIR) -f $@ -t $$IMAGE_NAME && \
		docker push $$IMAGE_NAME && \
		echo -n $$IMAGE_NAME > $(BUILD_DIR)/$${PROJECT_NAME}-image

clean:
	rm -rf $(BUILD_DIR)
