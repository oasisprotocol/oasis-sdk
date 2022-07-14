# Runtime Transaction Test Vectors

In order to test runtime transaction generation, parsing and signing, we provide
a set of runtime test vectors.

## Structure

The generated runtime test vectors file is a JSON document which provides an
array of objects (test vectors). Each test vector has the following fields:

* `kind` is a human-readable string describing what kind of a transaction the
  given test vector is describing (e.g., `"oasis-sdk runtime test vectors:
  consensus.Deposit"`).

* `tx` is the human-readable _interpreted_ unsigned transaction. Its purpose is
  to make it easier for the developer to understand what the content of the
  transaction is. For example `call.body` is represented as a readable JSON
  while in the encoded transaction this would be CBOR-encoded blob or an
  encrypted content.

* `meta` are meta-data sent to the hardware wallet containing `sig_context`,
  `runtime_id`, `chain_context` and optional `orig_to` field. `sig_context` is
  the [domain separation context] used for signing the transaction derived
  from `runtime_id` and `chain_context`. Check [chain context derivation] code
  for more information. `orig_to` is used by the hardware wallet to show a
  human-readable Ethereum address, because the transaction itself only contains
  the oasis native address.

* `signed_tx` is the human-readable signed transaction to make it easier for the
  developer to understand how the `call.body` and `ai` fields looks like in
  the [runtime transaction].

* `encoded_tx` and `encoded_meta` are CBOR-encoded (since test vectors are in
  JSON and CBOR encoding is a binary encoding it needs to be Base64-encoded)
  unsigned transaction and metadata respectively.

* `encoded_signed_tx` is the CBOR-encoded (since test vectors are in JSON and
  CBOR encoding is a binary encoding it needs to be Base64-encoded) signed
  transaction. **This is what is actually broadcast to the network.**

* `valid` is a boolean flag indicating whether the given test vector represents
  a valid transaction, including:

  * transaction having a valid signature,
  * transaction being correctly serialized,
  * transaction passing basic static validation.

  _NOTE: Even if a transaction passes basic static validation, it may still
  **not** be a valid transaction on the given network due to invalid nonce, or
  due to some specific parameters set on the network._

* `signer_private_key` is the Ed25519, Secp256k1 or Sr25519 private key that
  was used to sign the transaction in the test vector. The actual signature
  scheme is defined in `tx.ai.si[0].address_spec.signature`.

* `signer_public_key` is the Ed25519, Secp256k1 or Sr25519 public key
  corresponding to `signer_private_key`.

<!-- markdownlint-disable line-length -->
[chain context derivation]: https://github.com/oasisprotocol/oasis-sdk/blob/main/client-sdk/go/crypto/signature/context.go
[runtime transaction]: https://github.com/oasisprotocol/oasis-sdk/blob/488447a1f72c948a3437993cca9e3fd83bcfe617/runtime-sdk/src/types/transaction.rs#L86-L96
<!-- markdownlint-enable line-length -->
