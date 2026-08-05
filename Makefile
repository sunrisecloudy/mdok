.PHONY: fmt lint test corpus bench bench-perf profile-perf deps-audit sbom options
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
bench-perf:
	python3 scripts/bench_performance.py
profile-perf:
	sh scripts/profile_performance.sh
deps-audit:
	python3 scripts/audit_dependencies.py
sbom:
	python3 scripts/generate_sbom.py --output target/mdok.spdx.json
