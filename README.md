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

## Directories

* [`client-sdk`]: Client libraries for interacting with the Oasis consensus layer
  and runtimes in different languages.
* [`runtime-sdk`]: Oasis Runtime SDK that makes it easy to develop new runtimes.
* [`runtime-sdk/modules`]: A collection of larger modules for the Runtime SDK.
* [`contract-sdk`]: Oasis Contract SDK that can be used to develop Rust smart
  contracts that compile to WASM and can be executed inside the [`contracts`]
  Runtime SDK module.

[`client-sdk`]: client-sdk/
[`runtime-sdk`]: runtime-sdk/
[`runtime-sdk/modules`]: runtime-sdk/modules/
[`contract-sdk`]: contract-sdk/
[`contracts`]: runtime-sdk/modules/contracts/

## License

All Oasis SDK software components are licensed under [Apache 2.0](./LICENSE).

The content of the documentation (the `/docs` folder in any of the components)
including the media (e.g. images and diagrams) is licensed under [Creative
Commons Attribution 4.0 International](./LICENSE-docs).

