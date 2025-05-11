# Features

Containerized ROFL apps automatically have access to some useful features that
ease development. This chapter provides an introduction to these features.

## Secrets

Sometimes containers need access to data that should not be disclosed publicly,
for example API keys to access certain services. This data can be passed to
containers running inside ROFL apps via _secrets_. Secrets are arbitrary
key-value pairs which are end-to-end encrypted so that they can only be
decrypted inside a correctly attested ROFL app instance.

Secrets can be easily managed via the Oasis CLI, for example to create a secret
called `mysecret` you can use:

```sh
echo -n "my very secret value" | oasis rofl secret set mysecret -
```

Note that this only encrypts the secret and updates the local app manifest file,
but the secret is not propagated to the app just yet. This allows you to easily
configure as many secrets as you want without the need to constantly update the
on-chain app configuration.

:::info

While the secrets are stored in the local app manifest, this does not mean that
the manifest needs to remain private. The secret values inside the manifest are
end-to-end encrypted and cannot be read even by the administrator who set them.

When a secret is created, a new ephemeral key is generated that is used in the
encryption process. The ephemeral key is then immediately discarded so only the
ROFL app itself can decrypt the secret.

:::

Updating the on-chain configuration can be performed via the usual `update`
command as follows:

```sh
oasis rofl update
```

Inside containers secrets can be passed either via environment variables or via
container secrets.

### Environment Variables

Each secret is automatically exposed in the compose environment and can be
trivially used in the compose file. Note that when exposed as an environment
variable, the secret name is capitalized and spaces are replaced with
underscores, so a secret called `my secret` will be available as `MY_SECRET`.

```yaml
services:
  test:
    image: docker.io/library/alpine:3.21.2@sha256:f3240395711384fc3c07daa46cbc8d73aa5ba25ad1deb97424992760f8cb2b94
    command: echo "Hello $MYSECRET!"
    environment:
      - MYSECRET=${MYSECRET}
```

### Container Secrets

Each secret is also defined as a [container secret] and can be passed to the
container as such. Note that the secret needs to be defined as an _external_
secret as it is created by the ROFL app during boot.

```yaml
services:
  test:
    image: docker.io/library/alpine:3.21.2@sha256:f3240395711384fc3c07daa46cbc8d73aa5ba25ad1deb97424992760f8cb2b94
    command: echo "Hello $(cat /run/secrets/mysecret)!"
    secrets:
      - mysecret

secrets:
  mysecret:
    external: true
```

## Persistent Volumes

You can create persistent volumes and bind them to containers in an arbitrary
path in the filesystem. The volumes are created in an encrypted persistent disk.

```yaml
services:
  test:
    # ... other options here ...
    volumes:
      - my-volume:/path/to/my/volume

volumes:
  my-volume:
```

## ROFL REST APIs

Each containerized ROFL app runs a special daemon (called `rofl-appd`) that
exposes additional functions via a simple HTTP REST API. In order to make it
easier to isolate access, the API is exposed via a UNIX socket located at
`/run/rofl-appd.sock` which can be passed to containers via volumes.

For example:

```yaml
services:
  mycontainer:
    # ... other details omitted ...
    volumes:
      - /run/rofl-appd.sock:/run/rofl-appd.sock
```

The following sections describe the available endpoints.

### App Identifier

This endpoint can be used to retrieve the current ROFL app's identifier.

**Endpoint:** `/rofl/v1/app/id` (`GET`)

**Example response:**

```
rofl1qqn9xndja7e2pnxhttktmecvwzz0yqwxsquqyxdf
```

### Key Generation

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

### Authenticated Transaction Submission

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

- `encrypted` is a boolean flag specifying whether the transaction should be
  encrypted. By default this is `true`. Note that encryption is handled
  transparently for the caller using an ephemeral key and any response is first
  decrypted before being passed on.

**Example response:**

```json
{
  "data": "f6"
}
```
