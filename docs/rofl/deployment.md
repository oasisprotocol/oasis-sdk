# Deployment

ROFLs can be deployed to any ParaTime that has the ROFL module installed. In our
case we will be using the [Sapphire Testnet][sapphire] which has all the
required functionality.

[sapphire]: https://github.com/oasisprotocol/docs/blob/main/docs/build/sapphire/network.mdx

Your ROFL app will be deployed to a *ROFL node*. This is a light Oasis Node with
enabled support for TEE. There are two ways to deploy your app:

1. The preferred option is to rent a ROFL node using the [ROFL marketplace](#rofl-marketplace) and
   deploy your app directly via the Oasis CLI.
2. Alternatively, you can copy over the ROFL bundle to your private ROFL node
   instance and configure it. In this case, consult the [ROFL node] chapter.

[ROFL node]: ../../node/run-your-node/rofl-node.mdx

## ROFL Marketplace

The ROFL marketplace allows ROFL app developers on one side to easily and safely
deploy their apps for a small fee. On the other side it enables ROFL node
providers to lend their ROFL nodes for computation. The marketplace is
integrated into the Oasis Sapphire.

![The ROFL Marketplace](./images/rofl-marketplace.svg)

The Oasis CLI has built-in support for renting a machine on the ROFL marketplace
and deploying your app to it. To list offers of the default Oasis-managed ROFL
provider, run:

```shell
oasis rofl deploy --show-offers
```

```
Using provider: oasis1qp2ens0hsp7gh23wajxa4hpetkdek3swyyulyrmz (oasis1qp2ens0hsp7gh23wajxa4hpetkdek3swyyulyrmz)

Offers available from the selected provider:
- playground_short [0000000000000001]
  TEE: tdx | Memory: 4096 MiB | vCPUs: 2 | Storage: 19.53 GiB
```

You can select a different provider and offer by using the
[`--provider`][oasis-rofl-deploy] and [`--offer`][oasis-rofl-deploy] parameters
respectively.

For now, let's just go with defaults and execute: 

```shell
oasis rofl deploy
```

```
Using provider: oasis1qp2ens0hsp7gh23wajxa4hpetkdek3swyyulyrmz (oasis1qp2ens0hsp7gh23wajxa4hpetkdek3swyyulyrmz)
Pushing ROFL app to OCI repository 'rofl.sh/0ba0712d-114c-4e39-ac8e-b28edffcada8:1747909776'...
No pre-existing machine configured, creating a new one...
Taking offer: playground_short [0000000000000001]
```

The command above performed the following actions:

1. copied over ROFL bundle .orc to an Oasis-managed OCI repository `rofl.sh`,
2. paid an offer `playground_short` with ID `0000000000000001` at provider
  `oasis1qp2ens0hsp7gh23wajxa4hpetkdek3swyyulyrmz`,
3. obtained the machine ID and stored it to the manifest file.

You can now check the status of your rented ROFL machine by invoking:

```shell
oasis rofl machine show
```

```
Name:       default
Provider:   oasis1qp2ens0hsp7gh23wajxa4hpetkdek3swyyulyrmz
ID:         00000000000000a2
Offer:      0000000000000001
Status:     accepted
Creator:    oasis1qpupfu7e2n6pkezeaw0yhj8mcem8anj64ytrayne
Admin:      oasis1qpupfu7e2n6pkezeaw0yhj8mcem8anj64ytrayne
Node ID:    bOlqho9R3JHP64kJk+SfMxZt5fNkYWf6gdhErWlY60E=
Created at: 2025-05-22 15:01:47 +0000 UTC
Updated at: 2025-05-22 15:01:59 +0000 UTC
Paid until: 2025-05-22 16:01:47 +0000 UTC
Resources:
  TEE:     Intel TDX
  Memory:  4096 MiB
  vCPUs:   2
  Storage: 20000 MiB
Deployment:
  App ID: rofl1qpjsc3qplf2szw7w3rpzrpq5rqvzv4q5x5j23msu
  Metadata:
    net.oasis.deployment.orc.ref: rofl.sh/0ba0712d-114c-4e39-ac8e-b28edffcada8:1747909776@sha256:77ff0dc76adf957a4a089cf7cb584aa7788fef027c7180ceb73a662ede87a217
Commands:
  <no queued commands>
```

This shows you, if a ROFL node was registered on-chain to execute your ROFL
app, the expiration date, the machine provider and other details.

[oasis-rofl-deploy]: https://github.com/oasisprotocol/cli/blob/master/docs/rofl.md#deploy

## Checking That the ROFL App is Running

In order to check that the ROFL app is running and which nodes have registered
on chain to execute it, you can use the following command:

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

To check whether the oracle is actually working, you can use the prepared
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
