# Changelog

## Unreleased changes

New features:

- Hashing and many related functions that internally need to compute a hash,
  such as getting the address of a public key, are now declared as
  synchronous.
  We had implementations that used synchronous hashing libraries all along,
  but this is us giving up on eventually using the Web Crypto API for
  SHA-512/256.

Little things:

- We're switching lots of cryptography dependencies to noble cryptography
  libraries.

## v1.1.0

Spotlight change:

- Compatibility with oasis-core is updated to 23.0.10.
  There's a few new methods, structures, and changes to existing methods.

Little things:

- Our dependency on js-sha512 is bumped to 0.9.0, which corrects a bug that
  didn't affect our use of the library.
  However, this may affect other parts of your codebase if you depended on,
  for example, `"js-sha512": ">=0.8.0"`, and used it to hash strings.

## v1.0.0

Spotlight change:

- Bumped protobufjs to 7.2.x.
  Due to the versioning scheme they use, this is a "major" version, so
  downstream packages can now upgrade to that too.

Little things:

- Requests for scalar values (e.g. `consensusGetSignerNonce`) now properly
  return falsy values (e.g. 0) instead of `undefined`.
  Our thanks to the grpc-web team that helped us add support for this in their
  library!
- Compatibility with oasis-core is updated to 23.0.
  How neat was that, that nobody complained about incompatibility for so long?

Extra note:

- Happy v1!
  We'll be publishing this to the `latest` tag on npm, so you won't need to
  specify the `@alpha` tag anymore.

## v0.1.1-alpha.2

Spotlight change:

- We fixed an incompatibility with grpc-web 1.4.0.

Materials:

- Fixed a broken link to the account key generation specification.
- We've filled in the `homepage` field in our package.json files so now you
  can easily navigate from npm to GitHub.

## v0.1.1-alpha.1

Spotlight change:

- We've tightened up some TypeScript declarations to work better in strict
  mode.

## v0.1.0-alpha9

Spotlight change:

- Compatibility with oasis-core is updated to 22.1.5.

New features:

- The typing for `consensusGetSignerNonce` is corrected to indicate that it
  return `undefined` instead of `0`.

## v0.1.0-alpha8

Spotlight change:

- `client` methods additionally create a stack trace before making the
  request, to help you figure out what called the method in browsers that
  don't automatically hook up asynchronous stack traces.
  Which is most of them.

New features:

- Errors from `client` are now wrapped to show what method you were calling.
  Use the `.cause` property in newer browsers to get the original error.

## v0.1.0-alpha7

Spotlight change:

- We changed the protobuf scripts to be commonjs for better compatibility with
  various tools.

## v0.1.0-alpha6

Spotlight change:

- Errors from oasis-core with code and module now have `oasisCode` and
  `oasisModule` fields.

Breaking changes:

- The `NodeAddress` type is replaced with the equivalent `NetTCPAddr`.

New features:

- `oasis.misc` now has `toBase64`, `fromBase64`, and `toStringUTF8`.
- Errors from GRPC are now wrapped to include a stack trace.

Bug fixes:

- Fixes to type declarations that were inconsistent with the Go types.

## v0.1.0-alpha5

Spotlight change:

- Added `signature.visitMessage` for use in code that looks at a message
  before signing.

New features:

- Added `consensus.visitTransaction` for the case where that message to sign
  is a consensus transaction.

## v0.1.0-alpha4

Spotlight change:

- We need bech32 at runtime.
  Corrected that in our package.json.

## v0.1.0-alpha3

Spotlight change:

- Compatibility with oasis-core is updated to 21.2.1.

Breaking changes:

- It's official, `RootHash` is spelled with a capital "H."
  To celebrate, we're going to break all your references.
  Yay.

## v0.1.0-alpha2

Spotlight change:

- A new `hdkey` module implements ADR 0008 key generation.
  The implementation uses `Buffer` and `stream`, so you'll need the following
  in your configs if you use Webpack like we do:
  ```js
  {
      resolve: { fallback: { stream: require.resolve('stream-browserify') } },
      plugins: [
          new webpack.ProvidePlugin({
              process: 'process/browser',
              Buffer: ['buffer', 'Buffer'],
          }),
      ]
  }
  ```

New features:

- Compatibility with oasis-core is updated to 21.1.1.

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
