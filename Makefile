.PHONY: all check fmt clippy lint clean

all:
	cargo build --release

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
