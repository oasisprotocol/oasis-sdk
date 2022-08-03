# Changelog

## v0.2.1-alpha.2

Spotlight change:

- A new set of helpers is here to perform the encryption and decryption for
  confidential calls.

New features:

- We've brought in additions from the SDK itself, notably tooling for
  signed queries,
  changeable contract upgrade policy, and
  contract code storage.

## v0.2.1-alpha.1

Spotlight change:

- We added bindings for several new SDK features, notably including the
  contracts module, the gas used event, and runtime introspection.

New features:

- We've tightened up some TypeScript declarations to work better in strict
  mode.

## v0.2.0-alpha10

Spotlight change:

- `event.Visitor` now iterates through event arrays, which is newly added in
  the SDK itself.

## v0.2.0-alpha9

Spotlight change:

- We had an `evm` module all along but didn't export it.
  Now it's exported 🤦

Breaking changes:

- Some error code constants from the core module were missing the `_CODE` prefix.
  We've corrected that omission.

## v0.2.0-alpha8

Spotlight change:

- We've added new event visitor types for the consensus accounts module, to go
  with the newly added deposit/withdraw events in the SDK itself.

## v0.2.0-alpha7

Spotlight change:

- Fixes to `signatureSecp256k1.EllipticSigner` to match the Rust side.

## v0.2.0-alpha6

Spotlight change:

- Another addition from the SDK itself,
  `accounts.Wrapper.queryDenominationInfo` is here, and it's a "biiiiiiiiig"
  deal. That's 9 "i"s, if you wanted to know that too.

## v0.2.0-alpha5

Spotlight change:

- We usually don't itemize changes that come from the SDK itself in this
  client package's release notes, but the new `address.fromSigspec` is here,
  and it's kind of a big deal.

New features:

- You can now use SubmitTxNoWait from a transaction wrapper.
- Added `setRound` to query wrapper for convenient call chaining.

## v0.2.0-alpha4

Spotlight change:

- We made unitemized changes to track the (unversioned) runtime SDK itself.

Documentation changes:

- Added doc comments for some signing types and functions.

## v0.2.0-alpha3

Spotlight change:

- Added `transaction.SignatureMessageHandlersWithChainContext` for use with
  `oasis.signature.visitMessage`.

New features:

- `event.Visitor.visit` now returns whether it had a handler for the given
  event.
- Added `transaction.visitCall` for the case where that message is a runtime
  transaction.

## v0.2.0-alpha2

Spotlight change:

- We made unitemized changes to track the (unversioned) runtime SDK itself.

## v0.2.0-alpha1

Spotlight change:

- There's a new `event.Visitor` class to help set up event handlers with type
  help.
  Construct it with the outputs of modules' new `moduleEventHandler`
  functions.

Breaking changes:

- `event.toTag` is renamed to `toKey`.
  It never included the value, so that was only the key all along.

## v0.1.0-alpha1

Spotlight change:

- We'll now be putting this on npm.

Breaking changes:

- `TransactionWrapper.sign` now computes the signature context.
  Accordingly, it now takes the consensus chain context as a parameter.
  You can get this from `oasis.client.NodeInternal.consensusGetChainContext` if
  you don't already have it.

Note: nonbreaking changes made before v0.1.0 aren't catalogued.
Ask us directly or see the Git history for what changed.

## v0.0.2

Spotlight change:

- Runtime method wrappers now return a class with methods that let you sign
  and submit the transaction separately.
  This applies to query methods too, even though there's no signing step, for
  more minor reasons.

## v0.0.1

Spotlight change:

- Electrical anomalies have caused a Ctrl+V of the rest of this repository
  to be pasted as TypeScript instead of Rust.
