.PHONY: fmt lint test corpus bench
fmt:
	cargo fmt --all --check
lint:
	cargo clippy --workspace --all-targets --all-features -- -D warnings
corpus:
	python3 ../scripts/validate_corpus.py
test:
	cargo test --workspace --all-features
bench:
	cargo bench --workspace
