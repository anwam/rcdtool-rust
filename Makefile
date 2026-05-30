.PHONY: build release install clean test

build:
	cargo build

release:
	cargo build --release

install: release
	cargo install --path .

test:
	cargo test

clean:
	cargo clean

help:
	@echo "Available targets:"
	@echo "  build    - Build the project in debug mode"
	@echo "  release  - Build the project in release mode"
	@echo "  install  - Build and install the binary"
	@echo "  test     - Run tests"
	@echo "  clean    - Clean build artifacts"
	@echo "  help     - Show this help message"
