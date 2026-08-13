# Changelog

All notable changes to this project are documented in this file.

## [v0.1.0] - 2026-08-13

### Security

- **Encrypted filename padding**: filenames are now zero-padded to a fixed
  length before encryption, so identical-length names no longer leak the true
  filename length; names are also truncated at UTF-8 boundaries instead of
  failing on multi-byte characters (fixes an issue where long filenames were
  rejected).
- **Stricter Content-Security-Policy**: removed `unsafe-inline`/`unsafe-eval`
  script sources and third-party origins; the app now ships with
  `script-src 'self'`, explicit `font-src 'self'`, `frame-ancestors 'none'`,
  and `object-src 'none'`. No inline scripts exist in the built bundle.
- **No more Google Fonts**: fonts are now self-hosted in the binary, removing
  the `fonts.googleapis.com` / `fonts.gstatic.com` requests that leaked every
  visitor's IP address to Google.
- **SQLite hardening**: `secure_delete` enabled and the WAL checkpoint runs
  after the janitor sweep, reducing the chance of deleted file data lingering
  on disk.
- **Internal errors no longer leak details**: generic messages are returned to
  clients; full details only go to the server log.
- **Larger filename support**: maximum accepted filename length raised from
  255 to 2048 bytes (matching the new padding scheme).

### Changed

- Upload chunk size is now a per-upload server-side value (`chunk_size`) and is
  used consistently across upload, metadata, and download flows instead of a
  hardcoded constant.
- README documents the trust model and the `trust_proxy` configuration warning.

### CI / Release Infrastructure

- `ci.yml`: test matrix on ubuntu-24.04 and windows-2022, with format check and
  Clippy (deny warnings) gates and locked, dependency-pinned builds.
- `release.yml`: the Svelte UI is built once and reused; release binaries are
  built on native x86_64/arm64 runners (Linux, Windows, macOS) with sccache
  caching; binaries are signed with build provenance attestations and released
  via the GitHub CLI.
- Dependabot configured for GitHub Actions and npm dependencies (weekly,
  grouped).

### Dependencies

- `actions/attest-build-provenance` bumped 3 -> 4.
- UI dev dependencies (npm group) bumped.
