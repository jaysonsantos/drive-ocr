RUST_FILES := $(shell fd -e rs -e toml -e lock)

.PHONY = lint
lint: .cache/lint
	@echo Done

.PHONY = lint-fix
lint-fix: .cache/lint-fix
	@echo Done

.PHONY = test
test:
	@cargo test

.PHONY = build-all-targets
build-all-targets:
	$(MAKE) build-target TARGET=release ARCH=x86_64-unknown-linux-musl
	$(MAKE) build-target TARGET=release ARCH=aarch64-unknown-linux-musl

.PHONY = build-target
build-target: target/$(ARCH)/$(TARGET)/drive-ocr
	@echo Done

target/$(ARCH)/$(TARGET)/drive-ocr: $(RUST_FILES)
	@cargo zigbuild --$(TARGET) --target $(ARCH)
	@echo built $@

.PHONY = copy-all-targets
copy-all-targets: build-all-targets
	@mkdir -p .cache/docker-build/amd64
	@mkdir -p .cache/docker-build/arm64
	@cp target/x86_64-unknown-linux-musl/release/drive-ocr .cache/docker-build/amd64/
	@cp target/aarch64-unknown-linux-musl/release/drive-ocr .cache/docker-build/arm64/

.PHONY = build-docker
build-docker: copy-all-targets
	@mkdir -p .cache/docker-build
	@cd .cache/docker-build && \
	docker buildx build \
		-f ../../Dockerfile \
		-t ghcr.io/jaysonsantos/drive-ocr:$(DOCKER_VERSION) \
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
