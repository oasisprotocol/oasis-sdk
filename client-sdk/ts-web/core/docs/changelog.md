# Changelog

## v0.1.0-alpha1

Spotlight change:

- We'll now be putting this on npm.

Breaking changes:

- We're switching back to tweetnacl.
  Use `signature.NaclSigner` for similar functionality if you previously used
  `signature.EllipticSigner`.
  Use `NaclSigner.fromSeed` for similar functionality if you previously used
  `EllipticSigner.fromSecret`.

New features:

- There are now wrappers for consensus transactions, which help associate the
  method names with the right transaction body types.
- Compatibility with oasis-core is updated to 21.0.1.

Note: nonbreaking changes made before v0.1.0 aren't catalogued.
Ask us directly or see the Git history for what changed.

## v0.0.2

Spotlight change:

- `oasis.OasisNodeClient` is moved to `oasis.client.NodeInternal`.

## v0.0.1

Spotlight change:

- We've begrudgingly issued an actual version number for this package.
