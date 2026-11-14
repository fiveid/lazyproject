.PHONY: check test install-local setup-hooks clean

check:
	cargo fmt --all -- --check
	cargo clippy --all-targets --all-features -- -D warnings
	cargo test --all-targets --all-features

test:
	cargo test --all-targets --all-features

install-local:
	./scripts/local-release.sh

setup-hooks:
	git config core.hooksPath .githooks

clean:
	cargo clean
