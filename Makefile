.PHONY: fmt lint test corpus bench options
fmt:
	cargo fmt --all --check
lint:
	cargo clippy --workspace --all-targets --all-features -- -D warnings
corpus:
	python3 mdok-prd/scripts/validate_corpus.py
options:
	python3 scripts/sync_curl_options.py
test:
	cargo test --workspace --all-features
bench:
	cargo bench --workspace
