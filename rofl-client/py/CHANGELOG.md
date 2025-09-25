# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
