.PHONY: all test check fmt clippy lint clean

all:
	cargo build --release

test:
	cargo test

check:
	cargo check

fmt:
	cargo fmt

clippy:
	cargo clippy -- -D warnings

lint:
	cargo fmt && cargo clippy -- -D warnings

clean:
	cargo clean
