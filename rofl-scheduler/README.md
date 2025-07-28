# ROFL Scheduler

The ROFL Scheduler is a an app that allows providers to manage their nodes automatically based on
on-chain state. It is the off-chain counterpart to the ROFL Market module.

## App Identifiers

The scheduler is currently deployed on Sapphire Testnet and Mainnet with the following identifiers:

* Mainnet: `rofl1qr95suussttd2g9ehu3zcpgx8ewtwgayyuzsl0x2`
* Testnet: `rofl1qrqw99h0f7az3hwt2cl7yeew3wtz0fxunu7luyfg`

## Configuration

Using the scheduler first requires configuration which can be done through the Oasis Node
configuration file.

The following is an example configuration snippet for the scheduler.

```yaml
runtime:
    sgx_loader: /srv/node/bin/oasis-core-runtime-loader
    runtimes:
        - id: 000000000000000000000000000000000000000000000000a6d1e3ebf60dff6c
          components:
              # Regular Sapphire runtime configuration.
              - id: ronl
                config:
                    estimate_gas_by_simulating_contracts: true
                    allowed_queries:
                        - all_expensive: true
              # ROFL Scheduler app configuration.
              # This example uses the Testnet identifier.
              - id: rofl.rofl1qrqw99h0f7az3hwt2cl7yeew3wtz0fxunu7luyfg
                # The scheduler requires elevated permissions from Oasis Node.
                permissions:
                    - bundle_add
                    - bundle_remove
                    - volume_add
                    - volume_remove
                    - log_view
                # For best UX to deployed apps, some network configuration is needed.
                networking:
                    incoming:
                        # Incoming TCP port 443 needs to be forwarded to support the REST API
                        # and incoming proxy for deployed apps.
                        - ip: x.y.z.w
                          protocol: tcp
                          src_port: 443
                          dst_port: 443
                        # Incoming UDP port 4040 needs to be forwarded to support local Wireguard
                        # tunnels for deployed apps needed by the proxy.
                        - ip: x.y.z.w
                          protocol: udp
                          src_port: 4040
                          dst_port: 4040
                config:
                    rofl_scheduler:
                        # Address of the provider.
                        provider_address: oasis1qrfeadn03ljm0kfx8wx0d5zf6kj79pxqvv0dukdm
                        # Offers that this scheduler will accept.
                        offers:
                            - test
                        # Capacity of this node.
                        capacity:
                            # Maximum number of slots available for app deployment.
                            instances: 20
                            # Maximum amount of memory in megabytes available for app deployment.
                            memory: 131072
                            # Maximum amount of vCPUs available for app deployment.
                            cpus: 80
                            # Maximum amount of storage in megabytes available for app deployment.
                            storage: 1048576
                        # Domain that will be used for serving the REST API. It needs to be
                        # configured to point to the x.y.z.w IP address above.
                        api_domain: test-scheduler-a.rofl.app
                        # Optional proxy configuration.
                        proxy:
                            # Domain that will be used for the app proxy. It needs to be configured
                            # to point to the x.y.z.w IP address above.
                            domain: test-proxy-a.rofl.app
                            # External IP address for incoming Wireguard sessions used by the
                            # proxy.
                            external_wireguard_address: x.y.z.w
                            # Optional external IP address for incoming HTTPS proxy connections.
                            external_proxy_address: x.y.z.w
```

## Reproducible Build

You can reproduce the ROFL scheduler builds by running:

```
# Testnet.
oasis rofl build --deployment testnet --verify
# Mainnet.
oasis rofl build --deployment mainnet --verify
```

This will build the ORC and compare its identity with `rofl.yaml` and current on-chain policy.
