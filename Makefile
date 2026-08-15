.PHONY: fmt lint test corpus e2e-md mcp-conformance golden golden-update bench bench-perf profile-perf deps-audit sbom release-smoke tls-matrix options
fmt:
	cargo fmt --all --check
lint:
	cargo clippy --workspace --all-targets --all-features -- -D warnings
corpus:
	python3 mdok-prd/scripts/validate_corpus.py
e2e-md:
	python3 scripts/run_md_e2e.py
mcp-conformance:
	python3 scripts/run_mcp_conformance.py
golden:
	python3 scripts/run_golden_diff.py
golden-update:
	python3 scripts/run_golden_diff.py --update
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
release-smoke:
	sh scripts/run_release_smoke.sh
tls-matrix:
	python3 scripts/run_tls_matrix.py
