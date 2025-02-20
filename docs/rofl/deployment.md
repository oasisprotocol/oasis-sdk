# Deployment on Testnet and Mainnet

As a first step we need to decide which ParaTime the ROFL app will authenticate
to. This can be any ParaTime which has the ROFL module installed. For the rest
of this section we will be using [Sapphire Testnet][sapphire-testnet] which has
all of the required functionality.

[sapphire-testnet]: https://github.com/oasisprotocol/docs/blob/main/docs/build/sapphire/network.mdx

## Deploying the ROFL App Bundle

ROFL apps are deployed through Oasis nodes running on systems that support the
targeted TEE (e.g. Intel TDX). If you don't have a running node where you could
deploy your ROFL app, please first make sure that you have a client node with
the Sapphire Testnet runtime configured (see the [client node documentation] for
instructions on setting one up).

After your node is set up, include the built `myapp.default.orc`
[which we prepared earlier] in the `runtime` section in your configuration as
follows:

```yaml
runtime:
  # ... other options omitted ...
  paths:
    - /node/runtime/sapphire-paratime.orc
    - /node/runtime/myapp.default.orc
```

Note the appropriate paths to both the latest Sapphire Testnet runtime and the
ROFL app bundle. Before proceeding with the rest of the chapter, please make
sure that the node is fully synchronized with Sapphire Testnet.

The node will also need to cover any transaction fees that are required to
maintain registration of the ROFL app. First, determine the address of the node
you are connecting to by running the following:

```shell
oasis-node identity show-address -a unix:/node/data/internal.sock
```

This should output an address like the following:

```
oasis1qp6tl30ljsrrqnw2awxxu2mtxk0qxyy2nymtsy90
```

You can then [transfer some tokens] to this address on Sapphire Testnet to make
sure it will have funds to pay for registration fees:

```shell
oasis account transfer 10 oasis1qp6tl30ljsrrqnw2awxxu2mtxk0qxyy2nymtsy90 \
  --network testnet --paratime sapphire
```

[client node documentation]: https://github.com/oasisprotocol/docs/blob/main/docs/node/run-your-node/paratime-client-node.mdx#configuring-tee-paratime-client-node
[which we prepared earlier]: app.mdx#build
[transfer some tokens]: https://github.com/oasisprotocol/cli/blob/master/docs/account.md#transfer

## Checking That the ROFL App is Running

In order to check that the ROFL app is running and has successfully registered
on chain, you can use the following command:

```shell
oasis rofl show
```

This will output some information about the registered ROFL app, its policy and
its currently live instances:

```
App ID:        rofl1qqn9xndja7e2pnxhttktmecvwzz0yqwxsquqyxdf
Admin:         oasis1qrydpazemvuwtnp3efm7vmfvg3tde044qg6cxwzx
Staked amount: 10000.0 
Policy:
  {
    "quotes": {
      "pcs": {
        "tcb_validity_period": 30,
        "min_tcb_evaluation_data_number": 17,
        "tdx": {}
      }
    },
    "enclaves": [
      "z+StFagJfBOdGlUGDMH7RlcNUm1uqYDUZDG+g3z2ik8AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA==",
      "6KfY4DqD1Vi+H7aUn5FwwLobEzERHoOit7xsrPNz3eUAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=="
    ],
    "endorsements": [
      {
        "any": {}
      }
    ],
    "fees": 2,
    "max_expiration": 3
  }

=== Instances ===
- RAK:        AQhV3X660/+bR8REaWYkZNR6eAysFShylhe+7Ph00PM=
  Node ID:    DbeoxcRwDO4Wh8bwq5rAR7wzhiB+LeYn+y7lFSGAZ7I=
  Expiration: 9
```

Here you can see that a single instance of the ROFL app is running on the given
node, its public runtime attestation key (RAK) and the epoch at which its
registration will expire if not refreshed. ROFL apps must periodically refresh
their registrations to ensure they don't expire.

## Checking That the Oracle is Getting Updated

In order to check that the oracle is working, you can use the prepared
`oracle-query` task in the Hardhat project. Simply run:

```shell
npx hardhat oracle-query 0x1234845aaB7b6CD88c7fAd9E9E1cf07638805b20 --network sapphire-testnet
```

And you should get an output like the following:

```
Using oracle contract deployed at 0x1234845aaB7b6CD88c7fAd9E9E1cf07638805b20
ROFL app:  rofl1qqn9xndja7e2pnxhttktmecvwzz0yqwxsquqyxdf
Threshold: 1
Last observation: 63990
Last update at:   656
```

That's it! Your first ROFL oracle is running!
