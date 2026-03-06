# Test

Testing of your ROFL-powered applications and services depends on the [ROFL
flavor] you use.

[ROFL flavor]: ./init.md#rofl-flavors

## TDX ROFL containers

### No appd and Localnet Required

If your app doesn't rely on [appd] for deriving keys with KMS, submitting
ROFL-signed Sapphire transactions, managing ROFL metadata and other
ROFL-specific features, then you can simply export any secrets required by
`compose.yaml` and execute `podman compose up`. For example:

```shell
export SECRET=some_secret
podman compose up --build
```

### Testing with appd and Localnet

To test the behavior of your app connecting to a functional [appd], the
[`sapphire-localnet`] Docker image is able to expose a Localnet version of
the `rofl-appd.sock` UNIX socket for your containers to connect to and then
execute tests.

Simply spin up the [`sapphire-localnet`] Docker image and bind-mount the folder
that contains your `rofl.yaml`:

```shell
docker run -it -p8544-8548:8544-8548 -v .:/rofls ghcr.io/oasisprotocol/sapphire-localnet
```

In a few moments you should see an output like

```
* TDX ROFL detected. Localnet appd service will be accessible on the host via rofl-appd.sock in the shared volume
```

which means `rofl-appd.sock` will appear in the bind-mounted folder on your host
when the Localnet spins up.

Next, it's time to spin up your services and run tests. If you use the same
`compose.yaml` for production and testing, the following one-liner may come
handy to point a service to an alternate Localnet's `rofl-appd.sock` location:

```yaml title="compose.yaml"
    volumes:
      - ${ROFL_APPD_SOCKET:-/run/rofl-appd.sock}:/run/rofl-appd.sock
```

Now execute `podman compose` to bring up your services in the Localnet
environment:

```shell
export SECRET=some_secret
export ROFL_APPD_SOCKET=./rofl-appd.sock
podman compose up --build
```

Finally, you can try out the app or execute your end-to-end integration tests.
You can also use [`sapphire-localnet`] in form of a Github action inside your
[CI service].

[appd]: ../features/appd.md
[`sapphire-localnet`]: https://github.com/oasisprotocol/docs/blob/main/docs/build/tools/localnet.mdx
[CI service]: https://github.com/oasisprotocol/docs/blob/main/docs/build/tools/localnet.mdx#github-actions

## TDX ROFL raw

Testing ROFL TDX raw instances locally is currently not supported. You will need
to deploy them on Sapphire Testnet and try them out.

## SGX ROFL

Apps running in SGX ROFL are fully supported by the [`sapphire-localnet`] Docker
image.

First you need to compile your SGX ROFL with `debug` option enabled. Create a
separate `localnet` deployment in your ROFL manifest like so:

```yaml {7} title="rofl.yaml"
deployments:
  localnet:
    app_id: rofl1qqn9xndja7e2pnxhttktmecvwzz0yqwxsquqyxdf
    network: testnet
    paratime: sapphire
    admin: test:bob
    debug: true
```

and run

```shell
oasis rofl build --deployment localnet --offline
```

This will produce a bundle named `your-app-name.localnet.orc`.

Now execute the `sapphire-localnet` Docker image and bind-mount the folder that
contains the ROFL manifest. The image will automatically register the ROFL on
Localnet and execute it inside the Localnet ROFL node:

```shell
docker run -it -p8544-8548:8544-8548 -v .:/rofls ghcr.io/oasisprotocol/sapphire-localnet
```

If you see an output like

```
 * Detected SGX ROFL bundle: /rofls/rofl-appd-localnet.localnet.orc
```

then the Localnet .orc bundle was loaded and your ROFL will spin up for you to
test.
