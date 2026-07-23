# Public Variables

Sometimes containers need access to non-sensitive configuration, API endpoints,
contract addresses, feature flags, and other deliberately exposed information.
Such data can be passed to containers running in ROFL via _public variables_.
Public variables are arbitrary key-value pairs that are exposed to containers
as environment variables. For confidential values, use [secrets] instead.

Public variables can be easily managed via the Oasis CLI, for example to create
a public variable called `API_URL` you can use:

```sh
echo -n "https://api.example.com" | oasis rofl public-var set API_URL -
```

Note that this only updates the local app manifest file, but the public variable
is not propagated to the app just yet. This allows you to easily configure as
many public variables as you want without the need to constantly update the
on-chain app configuration.

Updating the on-chain configuration can be performed via the usual `update`
command as follows:

```sh
oasis rofl update
```

:::info Detailed CLI Reference

For comprehensive documentation on public variable management commands including
importing from `.env` files, removing public variables, and other advanced
features, consult the [Oasis CLI] documentation.

:::

Inside containers public variables can be passed via environment variables.

## Environment Variables

Each public variable is automatically exposed in the Compose environment and can
be trivially used in the Compose file.

```yaml
services:
  test:
    image: docker.io/library/alpine:3.21.2@sha256:f3240395711384fc3c07daa46cbc8d73aa5ba25ad1deb97424992760f8cb2b94
    command: echo "API URL is $API_URL"
    environment:
      - API_URL=${API_URL}
```

[secrets]: ./secrets.md
[Oasis CLI]: https://github.com/oasisprotocol/cli/blob/master/docs/rofl.md#public-var
