# 19. Release and Supply Chain

## 19.1 Artifacts

- macOS universal or separate arm64/x86_64 archives;
- Linux x86_64/aarch64 archives;
- Windows x86_64 ZIP;
- SHA-256 checksums;
- SBOM in SPDX or CycloneDX;
- build provenance/attestation;
- shell installer only after direct archive installation is stable.

## 19.2 Release checks

- Clean checkout build.
- Pinned curl checksum and patch verification.
- Full corpus and generated option-policy tests.
- Sanitizers and fuzz smoke.
- Benchmark comparison.
- `mdok version --json` feature snapshot.
- License/notice verification.
- Malware/secret scan of artifacts.
- Install-and-run smoke test in clean VMs/containers.

## 19.3 Compatibility promises

- Language version changes are explicit.
- JSON report schema follows additive evolution within a major schema version.
- Error codes are not reassigned.
- Curl compatibility version is printed and recorded in reports.
- A newer curl option is never silently accepted before classification.
