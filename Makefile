SHELL := /bin/bash

.PHONY: help build build-release run fmt clippy test clean

DEFAULT_PKG := ctmint-cli
ARGS ?=

help:
	@printf "Targets:\n"
	@printf "  build        Build workspace\n"
	@printf "  build-release Build release binaries\n"
	@printf "  run          Run ./target/release/ctmint (ARGS=...)\n"
	@printf "  fmt          Format code\n"
	@printf "  clippy       Lint (clippy)\n"
	@printf "  test         Run tests\n"
	@printf "  clean        Clean build artifacts\n"

build:
	cargo build --workspace --release

build-release:
	cargo build --workspace --release

run:
	$(MAKE) build-release
	@if [[ -z "$(strip $(ARGS))" ]]; then \
		./target/release/ctmint --help; \
	else \
		./target/release/ctmint $(ARGS); \
	fi

fmt:
	cargo fmt --all

clippy:
	cargo clippy --workspace --all-targets --all-features -- -D warnings

test:
	cargo test --workspace

clean:
	cargo clean
