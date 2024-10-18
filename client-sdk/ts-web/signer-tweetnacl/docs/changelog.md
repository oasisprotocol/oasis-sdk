# Changelog

## Unreleased changes

New features:

- This is the `NaclSigner` class formerly in `@oasisprotocol/client`. Feel
  free to continue using it for development.

Breaking changes:

- The `note` parameter is removed. Our opinion on using in-application-memory
  keys is unchanged. But if you're taking the step of installing this library
  called "signer-tweetnacl," you that it's using tweetnacl under the hood.
