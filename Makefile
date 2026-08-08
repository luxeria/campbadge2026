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

build:
	cargo build --release

format:
	cargo fmt --all --check

clippy:
	cargo clippy --all-features --workspace --tests --benches -- --deny warnings

doc:
	cargo doc --all-features --workspace --document-private-items --no-deps

test: format clippy
	cargo test --workspace --all-targets

flash: build
	espflash flash --monitor target/xtensa-esp32s3-espidf/release/firmware

clean:
	cargo clean
