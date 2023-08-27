ARTIFACTS=sk-vnode sk-cloudprov

include build/base.mk

$(ARTIFACTS):
	CGO_ENABLED=0 go build -trimpath -o $(BUILD_DIR)/$@ ./cmd/$@

lint:
	golangci-lint run

cover:
	go-carpet -summary
