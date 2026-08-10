.PHONY: \
	install \
	install-toolchain \
	install-flash \
	build \
	format \
	clippy \
	doc \
	test \
	flash \
	clean

install: install-toolchain install-flash

install-toolchain:
	@command -v espup >/dev/null 2>&1 || cargo install espup
	espup install

install-flash:
	@command -v espflash >/dev/null 2>&1 || cargo install espflash

GAME ?= dice

build:
	source "$$HOME/export-esp.sh" && \
	cd firmware && \
	cargo +esp build --release $(if $(filter dice,$(GAME)),,--no-default-features --features $(GAME))

format:
	cargo fmt --all --check

clippy:
	source "$$HOME/export-esp.sh" && \
	cargo +esp clippy --all-features --workspace --tests --benches -- --deny warnings

doc:
	cargo +esp doc --all-features --workspace --document-private-items --no-deps

test: format clippy
	source "$$HOME/export-esp.sh" && cargo +esp test --workspace --all-targets

# espflash auto-detects the board; append `--port <device>` only if more than one serial device
flash: build
	espflash flash --monitor target/xtensa-esp32s3-none-elf/release/firmware

clean:
	cargo clean
