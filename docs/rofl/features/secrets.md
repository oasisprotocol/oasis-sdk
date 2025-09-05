# Secrets

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

:::info Detailed CLI Reference

For comprehensive documentation on secret management commands including
importing from `.env` files, removing secrets, and other advanced features,
consult the [Oasis CLI] documentation.

:::

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

## Environment Variables

Each secret is automatically exposed in the Compose environment and can be
trivially used in the Compose file. Note that when exposed as an environment
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

## Container Secrets

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

[container secret]: https://docs.docker.com/compose/how-tos/use-secrets/
[Oasis CLI]: https://github.com/oasisprotocol/cli/blob/master/docs/rofl.md#secret
