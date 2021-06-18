# Oasis SDK

[![CI lint status][github-ci-lint-badge]][github-ci-lint-link]
[![CI audit status][github-ci-audit-badge]][github-ci-audit-link]
[![CI test status][github-ci-test-badge]][github-ci-test-link]
[![Rust coverage][codecov-badge]][codecov-link]

<!-- markdownlint-disable line-length -->
[github-ci-lint-badge]: https://github.com/oasisprotocol/oasis-sdk/workflows/ci-lint/badge.svg
[github-ci-lint-link]: https://github.com/oasisprotocol/oasis-sdk/actions?query=workflow:ci-lint+branch:main
[github-ci-audit-badge]: https://github.com/oasisprotocol/oasis-sdk/workflows/ci-audit/badge.svg
[github-ci-audit-link]: https://github.com/oasisprotocol/oasis-sdk/actions?query=workflow:ci-audit+branch:main
[github-ci-test-badge]: https://github.com/oasisprotocol/oasis-sdk/workflows/ci-test/badge.svg
[github-ci-test-link]: https://github.com/oasisprotocol/oasis-sdk/actions?query=workflow:ci-test+branch:main
[codecov-badge]: https://codecov.io/gh/oasisprotocol/oasis-sdk/branch/main/graph/badge.svg
[codecov-link]: https://codecov.io/gh/oasisprotocol/oasis-sdk
<!-- markdownlint-enable line-length -->

## Note

* **Oasis SDK is in active development so all APIs, protocols and data
  structures are subject to change.**
* **The SDK currently depends on v21.2.3 of [Oasis Core].**
* **The code has not yet been audited.**

[Oasis Core]: https://github.com/oasisprotocol/oasis-core
[local network]: https://docs.oasis.dev/oasis-core/development-setup/running-tests-and-development-networks/oasis-net-runner

## Directories

* [`client-sdk`]: Client libraries for interacting with the Oasis consensus layer
  and runtimes in different languages.
* [`runtime-sdk`]: Oasis Runtime SDK that makes it easy to develop new runtimes.

[`client-sdk`]: client-sdk/
[`runtime-sdk`]: runtime-sdk/
