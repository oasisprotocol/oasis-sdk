# `appd` REST API

Each containerized app running in ROFL runs a special daemon (called
`rofl-appd`) that exposes additional functions via a simple HTTP REST API. In
order to make it easier to isolate access, the API is exposed via a UNIX socket
located at `/run/rofl-appd.sock` which can be passed to containers via volumes.

An example using the [short syntax for Compose volumes][compose-volumes]:

```yaml
services:
  mycontainer:
    # ... other details omitted ...
    volumes:
      - /run/rofl-appd.sock:/run/rofl-appd.sock
```

## ROFL clients

After binding the UNIX socket above, we strongly suggest to access the ROFL
REST API through one of the ROFL clients for your favorite language:

- `oasis-rofl-client` for [Python]
- `@oasisprotocol/rofl-client` for [TypeScript]
- `oasis-rofl-client` for [Rust]

If you can't find the desired language above, please reach out to us on our
[#dev-central Discord channel][discord]. In the meantime use a generic HTTP
client and connect to the endpoints described next as follows.

:::info UNIX sockets and HTTP headers

Although the communication with `rofl-appd` is through UNIX sockets, the REST
service still uses the HTTP protocol. In place of a host name you can provide
any name. In our examples, we stick to the `http://localhost/<endpoint_path>`
format.

:::

[Python]: https://pypi.org/project/oasis-rofl-client/
[TypeScript]: https://www.npmjs.com/package/@oasisprotocol/rofl-client
[Rust]: https://github.com/oasisprotocol/oasis-sdk/tree/main/rofl-client/rs
[compose-volumes]: https://docs.docker.com/reference/compose-file/services/#short-syntax-5
[discord]: https://oasis.io/discord

## Endpoints

### App Identifier

This endpoint can be used to retrieve the app ID.

**Endpoint:** `/rofl/v1/app/id` (`GET`)

**Example response:**

```
rofl1qqn9xndja7e2pnxhttktmecvwzz0yqwxsquqyxdf
```

### Key Generation

Each registered app automatically gets access to a decentralized on-chain key
management system.

All generated keys can only be generated inside properly attested app instances
and will remain the same even in case the app is deployed somewhere else or its
state is erased.

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

### Authenticated Transaction Submission

An app running in ROFL can also submit _authenticated transactions_ to the chain
where it is registered at. The special feature of these transactions is that
they are signed by an **endorsed ephemeral key** and are therefore automatically
authenticated as coming from the app itself.

This makes it possible to easily authenticate the transaction origin in smart
contracts by simply invoking an [appropriate subcall]:

```solidity
Subcall.roflEnsureAuthorizedOrigin(roflAppID);
```

[appropriate subcall]: https://api.docs.oasis.io/sol/sapphire-contracts/contracts/Subcall.sol/library.Subcall.html#roflensureauthorizedorigin

**Endpoint:** `/rofl/v1/tx/sign-submit` (`POST`)

**Example request:**

```json
{
  "encrypt": true,
  "tx": {
    "kind": "eth",
    "data": {
      "gas_limit": 200000,
      "to": "1234845aaB7b6CD88c7fAd9E9E1cf07638805b20",
      "value": "0",
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

    - `gas_limit` may be provided either as a JSON number (e.g. `21000`) or as
      a decimal string or `0x`-prefixed hex string. All forms are interpreted
      as a non-negative 64-bit integer and must not contain whitespace.
    - `value` must represent a non-negative integer up to 256 bits and may be
      provided as a decimal string, a `0x`-prefixed hex string, or as a JSON
      number up to `2^64 - 1`. String forms must not contain whitespace.
    - Hex-encoded fields such as `to` and `data` accept strings with or without
      a leading `0x` prefix, must not contain whitespace, and must not be
      prefix-only (`"0x"`). Use an empty string (`""`) to represent empty bytes
      (e.g. `to: ""` for contract creation, `data: ""` for empty calldata).
      When `to` is non-empty it must decode to exactly 20 bytes (an Ethereum
      address).

  - Oasis SDK calls (`std`) support CBOR-serialized hex-encoded `Transaction`s
    to be specified.

- `encrypt` is a boolean flag specifying whether the transaction should be
  encrypted. By default this is `true`. Note that encryption is handled
  transparently for the caller using an ephemeral key and any response is first
  decrypted before being passed on.

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

### Replica Metadata

Replica metadata allows apps to publish arbitrary key-value pairs that are
included in the on-chain ROFL replica registration. This metadata is
automatically namespaced with `net.oasis.app.` when published on-chain.

#### Get Metadata

Retrieve all user-set metadata key-value pairs.

**Endpoint:** `/rofl/v1/metadata` (`GET`)

**Example response:**

```json
{
  "key_fingerprint": "a54027bff15a8726",
  "version": "1.0.0"
}
```

#### Set Metadata

Set metadata key-value pairs. This replaces all existing app-provided metadata
and will trigger a registration refresh if the metadata has changed.

**Endpoint:** `/rofl/v1/metadata` (`POST`)

**Example request:**

```json
{
  "key_fingerprint": "a54027bff15a8726",
  "version": "1.0.0"
}
```

**Note:** Metadata is validated against runtime-configured limits for the
number of pairs, key size, and value size.

### Query

Runs arbitrary query method defined in the [Oasis Runtime SDK] module and
returns the result.

**Endpoint:** `/rofl/v1/query` (`POST`)

**Example request:**

```json
{
  "method": "rofl.App",
  "args": "a16269645500694cb01f85408d624ea267f657bf285787a61db3"
}
```

Above we called the [`rofl.App`] method and passed the CBOR-encoded ROFL app ID
`rofl1qrl7nqwvaledwkw3n75jd7htklvsczje554u4p6k` (`a1` (map) + `626964` (key
"id") + `55` (byte string, 21 bytes) + app ID in hex).

[`rofl.App`]: https://github.com/oasisprotocol/oasis-sdk/blob/394da333625a189abd8b752a9f2dc46bb883a781/runtime-sdk/src/modules/rofl/mod.rs#L647

**Request fields:**

- `method`: The internal name of the method. Query methods are those having the
  `#[handler(query = "...")]` annotation in the [Oasis Runtime SDK] source.
- `args`: Query parameters for the method above serialized as CBOR and
  hex-encoded.

[Oasis Runtime SDK]: https://github.com/oasisprotocol/oasis-sdk/tree/main/runtime-sdk

**Example response:**

```json
{"data":"a76269645500ffe981cceff2d759d19fa926faebb7d90c0a59a56373656b582056c6d4841fa5ad24ad761088cb7053ad312ef13f3c2f405640fdba2bde4c14386561646d696e55007814f3d954f41b6459eb9e4bc8fbc6767ece5aa9657374616b658249056bc75e2d631000004066706f6c696379a56466656573026671756f746573a263696173f663706373a363746478a173616c6c6f7765645f7464785f6d6f64756c657380737463625f76616c69646974795f706572696f64181e781e6d696e5f7463625f6576616c756174696f6e5f646174615f6e756d6265721268656e636c6176657382a2696d725f7369676e6572582000000000000000000000000000000000000000000000000000000000000000006a6d725f656e636c6176655820bd4844a79a12ba365e890ddeaebbc4e4292797c7d956b42c2e25e4aefce3b124a2696d725f7369676e6572582000000000000000000000000000000000000000000000000000000000000000006a6d725f656e636c6176655820412c94a9baa0949f718dbba41ab89c38ea5c320345bba36b07e2ef857ffe2fb96c656e646f7273656d656e747381a163616e79a06e6d61785f65787069726174696f6e036773656372657473a26a50494e4154415f4a5754590327a462706b58207f88546291174f854a8ce2eb4bbfc8e62e40d60cdae2d600920a766d3006bf6c646e616d65581aad0460d0f742ad697b6d7c8462cc2b153deeb051a791553ef52b656e6f6e63654f8cbbe6631910d9a702e49bd30ee8f46576616c75655902c1352d823e54f75c846579066e2c1a750387f0630e3f29dba5524984712883bb33371ff017e507ae432b135312af64bfb85f3b17def05b2ac2744256f5accf1a26b29cd8cd412f08bfc9204f9bfa670b2d65972cfc4a8d4e2074402f21c18dfe554b1f0a8a731c077699f741807b3a4047ef4dc570958b8b46111a445259e93c9b92a5f72a23d32cbbef875efe586a8ddda38f1fa286d19b369a2022c3eae1cdbf6f6de4dc055bbab36a2c4830f8e2c64437f5f878f419e21f3a4c0f13c47b63697668b34722eaa61bdffa12e82be6d0266a41590254c9d70e9237475f6115c2867065c49afa8032acdfe0bc5dc671ef48ebc58d893937659243479e2eaa38815cd8665541e4c7e40349524cda15f2d410cba100ee27f0a59dc63534f7cff2444b57c7b74060cf8c3e21e1590b597384a89a463d6bb4a52fc52a4592889448a8f8e0c02fef1689c1efea58ddc08783f6d22d7a908cdf2a45bc46a79a3b73ff5f13bbbf7219973a7382eee84b3b4d48036b5e87fb24223e6387a3e37f9c1ac9722534adfef0201ec13db2e70fe9cd0336f020e50b59d7d32951378f568a4ef3a8117386ff0f1ba4b7d33dcf913daa696a6a7d64d20220b0e5eec994ea56aa9c01f56f5e12bf7d555b3f4a218ddbd8ae1586d5cb1e76802c90e26472b233686e8449284829378163acd77d1b65d4953a76cc497584580c1a5ac4fe6d97fd818fa53c2eb43c6f1b7a79211ef57f24036b1f8ed37f4c3d36873bbbd876769a5fde17add6926317afc96c9707e10b150735c63877ffd6475b1a27b6dd108940f0097375e422b1d308c6c8a0a83847368f5760778c54f2a70b44f9365c55c4b5d69341cad62d6fdb11aa25a9e26b4977adebd65718042bda1cbbf988298d293547107c2f5899352188179bc4d22febc66e8e351885498e707dee583d052c0b7c13f930e4529b59c36c20ee3ee6d29d1a3e2bb65bd489b7206cd45d2ae046f1e147d6407c166e494e465552415f4150495f4b45595899a462706b582074be2e227be81a5e35943ac1b79e238395b9bc367f7a0d1eed740c259ae23a37646e616d65581ee5190cda87688753f6f221890bfed8fe0e27b3ced4a9bc54537da1f4cd90656e6f6e63654fcf5411193abf4f4711f17179ea4b6f6576616c756558304675a29c8398bc94b5057063b4badeb9937a6e019ef7f8095d6b5a035106d4e8e7d710af9b6883848886481c1d180cbc686d65746164617461a7736e65742e6f617369732e726f666c2e6e616d656f76616c696461746f722d6167656e74756e65742e6f617369732e726f666c2e617574686f72782a4d61746576c5be204a656b6f766563203c6d617465767a406f6173697370726f746f636f6c2e6f72673e766e65742e6f617369732e726f666c2e6c6963656e73656a4170616368652d322e30766e65742e6f617369732e726f666c2e76657273696f6e65302e312e30776e65742e6f617369732e726f666c2e686f6d6570616765784f68747470733a2f2f6769746875622e636f6d2f6f6173697370726f746f636f6c2f6572632d383030342f626c6f622f6d61737465722f524541444d452e6d642376616c696461746f722d6167656e7478196e65742e6f617369732e726f666c2e7265706f7369746f7279782968747470733a2f2f6769746875622e636f6d2f6f6173697370726f746f636f6c2f6572632d38303034781a6e65742e6f617369732e726f666c2e6465736372697074696f6e787f4c697374656e7320746f2056616c69646174696f6e52657175657374206576656e7473206f6620746865204552432d383030342076616c69646174696f6e20726567697374727920616e642076616c696461746573207768657468657220746865206167656e7420697320706f776572656420627920524f464c205445452e"}
```

Inside `data` the JSON response contains the CBOR-serialized method's return
value in hex format.

:::example

Check out [this chunk][rofl-demo] of the ROFL demo repository for querying
with `curl` directly.

For a production-ready Python example, check out the [ROFL-8004 implementation]
where the `query` endpoint is used to fetch various app on-chain metadata for
registration in the [ERC-8004 identity registry].

:::

[rofl-demo]: https://github.com/oasisprotocol/demo-rofl/blob/ab7e60aeb5f10aaec0a5f401086b2ba259a30107/docker/app.sh#L9-L18
[ROFL-8004 implementation]: https://github.com/oasisprotocol/erc-8004/blob/18f8630f7397ec889ea55b008391664f3b736128/rofl-8004/rofl_metadata.py#L47
[ERC-8004 identity registry]: https://eips.ethereum.org/EIPS/eip-8004#identity-registry
