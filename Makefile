.PHONY: test lint lint-strict format format-check build c-header

all: test lint format

test:
	$(info ****************** running tests ******************)
	cargo test --all

lint-strict:
	$(info ****************** running clippy in strict mode ******************)
	cargo clippy --all-targets --all-features -- -D warnings  # treat warnings as errors

lint:
	$(info ****************** running clippy in strict mode ******************)
	cargo clippy --all-targets --all-features

lint-fix:
	$(info ****************** running clippy in strict mode ******************)
	cargo clippy --fix --allow-dirty --all-targets --all-features

format:
	$(info ****************** formatting ******************)
	cargo fmt --all

format-check:
	$(info ****************** checking formatting ******************)
	cargo fmt --all --check

build:
	$(info ****************** building ******************)
	cargo build

c-header:
	$(info ****************** generating C header ******************)
	cbindgen --config crates/liteparse-c/cbindgen.toml --crate liteparse-c --output crates/liteparse-c/include/liteparse.h
