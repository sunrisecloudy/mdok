# ADR 0002: Vendor curl Tool Parser and libcurl

Status: Accepted

libcurl alone does not parse a pasted curl command line. MDOK vendors a pinned curl source release, exposes the real tool parser through a narrow C bridge, and uses libcurl for execution. This avoids an incomplete reimplementation while isolating unstable curl internals from Rust.
