# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog], and this project adheres to [Semantic Versioning].

## [Unreleased]

### Added
- 

### Changed
- 

### Fixed
- 

## 0.1.0 - 2025-10-21

### Added
- Initial public release of the Rust ROFL client (`oasis-rofl-client`).
- Core API:
  - `RoflClient::get_app_id`
  - `RoflClient::generate_key` with `KeyKind` (`raw-256`, `raw-384`, `ed25519`, `secp256k1`)
  - `RoflClient::sign_submit` (supports `eth` and `std` tx kinds)
  - `RoflClient::sign_submit_eth` convenience helper
  - `RoflClient::get_metadata` getter for Metadata entries
  - `RoflClient::set_metadata` setter for Metadata entries
  - `RoflClient::query` on-chain data queries
- HTTP-over-UDS transport targeting `/run/rofl-appd.sock` with async methods offloading blocking I/O.
- Example: `examples/basic.rs`.
- README with quickstart.

[Keep a Changelog]: https://keepachangelog.com/en/1.0.0/
[Semantic Versioning]: https://semver.org/spec/v2.0.0.html