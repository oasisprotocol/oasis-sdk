# `appd` REST API

Each containerized ROFL app runs a special daemon (called `rofl-appd`) that
exposes additional functions via a simple HTTP REST API. In order to make it
easier to isolate access, the API is exposed via a UNIX socket located at
`/run/rofl-appd.sock` which can be passed to containers via volumes.

An example using the [short syntax for Compose volumes][compose-volumes]:

```yaml
services:
  mycontainer:
    # ... other details omitted ...
    volumes:
      - /run/rofl-appd.sock:/run/rofl-appd.sock
```

The following sections describe the available endpoints.

:::info UNIX sockets and HTTP headers

Although the communication with `rofl-appd` is through UNIX sockets, the REST
service still uses the HTTP protocol. In place of a host name you can provide
any name. In our examples, we stick to the `http://localhost/<endpoint_path>`
format.

:::

[compose-volumes]: https://docs.docker.com/reference/compose-file/services/#short-syntax-5

## App Identifier

This endpoint can be used to retrieve the current ROFL app's identifier.

**Endpoint:** `/rofl/v1/app/id` (`GET`)

**Example response:**

```
rofl1qqn9xndja7e2pnxhttktmecvwzz0yqwxsquqyxdf
```

## Key Generation

Each registered ROFL app automatically gets access to a decentralized on-chain
key management system.

All generated keys can only be generated inside properly attested ROFL app
instances and will remain the same even in case the app is deployed somewhere
else or its state is erased.

**Endpoint:** `/rofl/v1/keys/generate` (`POST`)

**Example request:**

```json
{
  "key_id": "demo key",
  "kind": "secp256k1"
}
```

**Request fields:**

- `key_id` is used for domain separation of different keys (e.g. a different key
  id will generate a completely different key).

- `kind` defines what kind of key should be generated. The following values are
  currently supported:

  - `raw-256` to generate 256 bits of entropy.
  - `raw-386` to generate 384 bits of entropy.
  - `ed25519` to generate an Ed25519 private key.
  - `secp256k1` to generate a Secp256k1 private key.

**Example response:**

```json
{
  "key": "a54027bff15a8726b6d9f65383bff20db51c6f3ac5497143a8412a7f16dfdda9"
}
```

The generated `key` is returned as a hexadecimal string.

## Authenticated Transaction Submission

A ROFL app can also submit _authenticated transactions_ to the chain where it is
registered at. The special feature of these transactions is that they are signed
by an endorsed key and are therefore automatically authenticated as coming from
the ROFL app itself.

This makes it possible to easily authenticate ROFL apps in smart contracts by
simply invoking an [appropriate subcall], for example:

```solidity
Subcall.roflEnsureAuthorizedOrigin(roflAppID);
```

[appropriate subcall]: https://api.docs.oasis.io/sol/sapphire-contracts/contracts/Subcall.sol/library.Subcall.html#roflensureauthorizedorigin

**Endpoint:** `/rofl/v1/tx/sign-submit` (`POST`)

**Example request:**

```json
{
  "tx": {
    "kind": "eth",
    "data": {
      "gas_limit": 200000,
      "to": "1234845aaB7b6CD88c7fAd9E9E1cf07638805b20",
      "value": 0,
      "data": "dae1ee1f00000000000000000000000000000000000000000000000000002695a9e649b2"
    }
  }
}
```

**Request fields:**

- `tx` describes the transaction content with different transaction kinds being
  supported (as defined by the `kind` field):

  - Ethereum-compatible calls (`eth`) use standard fields (`gas_limit`, `to`,
    `value` and `data`) to define the transaction content.

  - Oasis SDK calls (`std`) support CBOR-serialized hex-encoded `Transaction`s
    to be specified.

- `encrypt` is a boolean flag specifying whether the transaction should be
  encrypted. By default this is `true`. Note that encryption is handled
  transparently for the caller using an ephemeral key and any response is first
  decrypted before being passed on.

- `evm_use_oasis_tx` is a boolean flag that controls transaction encoding for
  Ethereum-compatible calls. By default this is `false`, meaning `eth`
  transactions are encoded as native Ethereum transactions.

**Example response:**

Inside `data` the JSON response contains a CBOR-serialized hex-encoded
[call result]. To investigate it you will need to deserialize it first.

For example:

- Successful call result:

  ```json
  {
    "data": "a1626f6b40"
  }
  ```

  deserialized as `{"ok": ''}`.

- Unsusccessful call result:

  ```json
  {
    "data": "a1646661696ca364636f646508666d6f64756c656365766d676d6573736167657272657665727465643a20614a416f4c773d3d"
  }
  ```

  deserialized as
  `{"fail": {"code": 8, "module": "evm", "message": "reverted: aJAoLw=="}}`.

[call result]: https://api.docs.oasis.io/rust/oasis_runtime_sdk/types/transaction/enum.CallResult.html
