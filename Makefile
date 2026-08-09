.PHONY: \
	install \
	install-toolchain \
	install-flash \
	build \
	format \
	clippy \
	test \
	flash \
	clean

install: install-toolchain install-flash

install-toolchain:
	@command -v espup >/dev/null 2>&1 || cargo install espup
	espup install

install-flash:
	@command -v espflash >/dev/null 2>&1 || cargo install espflash

# The xtensa target is set in firmware/.cargo/config.toml, so the build must
# run from firmware/; the source of `~/export-esp.sh` is shell-level setup.
build:
	cd firmware && cargo build --release

format:
	cargo fmt --all --check

clippy:
	cargo clippy --all-features --workspace --tests --benches -- --deny warnings

test: format clippy
	cargo test --workspace --all-targets

# Requires the Xtensa toolchain (see install-toolchain / AGENTS.md). espflash
# auto-detects the board; append `--port <device>` only if more than one
# serial device could otherwise be ambiguous.
flash: build
	espflash flash --monitor target/xtensa-esp32s3-none-elf/release/firmware

clean:
	cargo clean
