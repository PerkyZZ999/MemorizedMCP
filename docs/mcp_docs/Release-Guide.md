# Release Guide

## Versioning

- Use semantic versioning MAJOR.MINOR.PATCH
- Tag releases in git: `git tag -a vX.Y.Z -m "Release vX.Y.Z" && git push --tags`

## Pre-Release Checklist

- `cargo build` and `cargo test` pass with no warnings
- Benchmarks run: `cargo bench` (record p50/p95)
- Update `docs/Tasks2.md` and changelog
- Verify `/status` health is `ok` under typical load
- Verify backup/restore with `system.backup` and `system.restore`

## Packaging

- Build binary: `cargo build --release`
- Publish artifact (zip): include `memory_mcp_server.exe`, `README.md`, example `.env`

## Post-Release

- Create GitHub Release, attach artifacts
- Document effective configuration and default thresholds
- Monitor metrics via Prometheus/Grafana
