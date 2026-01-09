# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.2.0 - 2026-01-09

### Added
- Non-async version of `RoflClient`

### Changed
- `RoflClient` is now the default non-async client. Use `AsyncRoflClient` if
  you need to preserve the existing async behavior and backward compatibility.

## 0.1.7 - 2025-12-03

### Added
- Support for `get_app_id` endpoint
- Support for `sign_submit` endpoint (requires `web3` dependency)

### Changed
- Use sphinx pydoc format for documentation consistency

### Fixed
- Release workflow when multiple tags exist on the same commit
- `sign_submit` now sends `value` as string (required by rofl-container 0.8.5+)

## 0.1.6 - 2025-11-06

### Fixed
- Don't try to parse `set_metadata` response

## 0.1.5 - 2025-11-01

### Added
- Queries support
- Appd metadata support (`get_metadata`, `set_metadata`)

## 0.1.4 - 2025-09-25

### Fixed
- set hatch root, to see git tags correctly.
- remove publish badge from README.md

## 0.1.3 - 2025-09-24

### Fixed
- stripping namespace correctly from version tag in workflow and hatch versioning

## 0.1.2 - 2025-09-22

### Fixed
- Release workflow validation error: removed invalid `working-directory` from `uses:` steps and set artifact `path` to `rofl-client/py/dist/`.


## 0.1.1 - 2025-09-18

### Fixed 
- Missing working directory in GitHub workflow


## 0.1.0 - 2025-09-17

### Added
- Initial public release
- Core ROFL client functionality for Python
- Documentation and usage examples
