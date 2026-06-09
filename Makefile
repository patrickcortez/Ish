.PHONY: install build test clean uninstall run setfont

setfont:
	cargo run --bin setfont

install:
	cargo install --path .

build:
	cargo build --release

test:
	cargo test

clean:
	cargo clean

uninstall:
	cargo uninstall ish

run:
	cargo run
