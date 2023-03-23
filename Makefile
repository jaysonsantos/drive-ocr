RUST_FILES := $(shell fd -e rs -e toml -e lock)
DOCKER := docker
DOCKER_IMAGE = ghcr.io/jaysonsantos/drive-ocr:$(DOCKER_VERSION)

RELEASE_ARM = target/aarch64-unknown-linux-gnu/release/drive-ocr
DOCKER_BINARY_ARM = .cache/docker-build/arm64/drive-ocr

RELEASE_AMD = target/x86_64-unknown-linux-gnu/release/drive-ocr
DOCKER_BINARY_AMD = .cache/docker-build/amd64/drive-ocr

ifeq ($(shell uname -m),arm64)
LOCAL_RELEASE := $(RELEASE_ARM)
else
LOCAL_RELEASE := $(RELEASE_AMD)
endif
RELEASES = $(RELEASE_AMD) $(RELEASE_ARM)

.PHONY = lint
lint: .cache/lint
	@echo Done

.PHONY = lint-fix
lint-fix: .cache/lint-fix
	@echo Done

.PHONY = test
test:
	@cargo test

$(RELEASES): $(RUST_FILES)
	@rustup target add $(shell echo $@ | cut -d/ -f2)
	@cargo zigbuild --release --target $(shell echo $@ | cut -d/ -f2)
	@echo built $@

.PHONY = build-arm
build-arm: $(RELEASE_ARM)
build-amd: $(RELEASE_AMD)
.PHONY = copy-all-targets
copy-all-targets: .cache/docker-build/arm64/drive-ocr .cache/docker-build/amd64/drive-ocr
.cache/docker-build/arm64/drive-ocr: $(RELEASE_ARM) .cache
	cp $< $@
.cache/docker-build/amd64/drive-ocr: $(RELEASE_AMD) .cache
	cp $< $@

.PHONY = build-docker
build-docker: copy-all-targets .cache/ghcr-login
	@mkdir -p .cache/docker-build
	cd .cache/docker-build && \
	cp ../../Dockerfile . && \
	$(DOCKER) buildx build \
		-t $(DOCKER_IMAGE) \
		--platform linux/amd64,linux/arm64 \
		--push \
		.

.PHONY = build-local-docker
build-local-docker: .cache/docker-build/arm64/drive-ocr
	@mkdir -p .cache/docker-build
	cd .cache/docker-build && \
	cp ../../Dockerfile . && \
	$(DOCKER) buildx build \
		-t $(DOCKER_IMAGE) \
		--platform $(DOCKER_PLATFORM) \
		--push \
		.

.cache:
	@mkdir $@

.cache/lint: .cache $(RUST_FILES)
	cargo fmt --check
	cargo clippy
	@touch $@

.cache/lint-fix: .cache $(RUST_FILES)
	cargo fmt
	cargo clippy --fix --allow-staged --allow-dirty
	@touch $@

.cache/ghcr-login: .cache
	@echo $(GITHUB_TOKEN) | podman login --username $(GITHUB_USER) --password-stdin ghcr.io
	@touch $@
