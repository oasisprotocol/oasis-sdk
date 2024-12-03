# Consensus Trust Root

The [ROFL app example] already contains an embedded root of trust called the
_consensus trust root_. The preconfigured trust root is valid for the current
deployment of Sapphire Testnet. This chapter briefly describes what the trust
root is, how it can be securely derived and configured in your ROFL app.

[ROFL app example]: app.mdx

## The Root of Trust

In order to ensure that the ROFL app can securely authenticate it is talking to
the actual consensus layer and not a fork, you need to configure it with a
suitable consensus trust root. The trust root is a well-known consensus block
header that is used to bootstrap the light client which verifies everything
else.

Having a correct trust root configured makes it impossible even for the node
operator who is running your ROFL app to forge any queries as integrity of all
results is verified against the consensus layer trust root. One should try to
use a somewhat recent trust root which can be refreshed during application
upgrades.

:::caution

The consensus trust root represents the root of security for the ROFL app. Not
configuring it makes your application vulnerable to man-in-the-middle attacks by
the node operator.

:::

## Embedding the Root

In order to obtain a suitable consensus trust root you can leverage the Oasis
CLI as follows (note the correct _network_ and _paratime_). Note that you should
use a node you trust to query this information (the example below uses the
default public gRPC endpoints).

```bash
oasis rofl trust-root --network testnet --paratime sapphire
```

Which should output something like the following:

```rust
TrustRoot {
    height: 22110615,
    hash: "95d1501f9cb88619050a5b422270929164ce739c5d803ed9500285b3b040985e".into(),
    runtime_id: "000000000000000000000000000000000000000000000000a6d1e3ebf60dff6c".into(),
    chain_context: "0b91b8e4e44b2003a7c5e23ddadb5e14ef5345c0ebcb3ddcae07fa2f244cab76".to_string(),
}
```

You can then use this in your ROFL app definition by adding the following method
to your `App` trait implementation.

```rust
fn consensus_trust_root() -> Option<TrustRoot> {
    Some(TrustRoot {
        height: 22110615,
        hash: "95d1501f9cb88619050a5b422270929164ce739c5d803ed9500285b3b040985e".into(),
        runtime_id: "000000000000000000000000000000000000000000000000a6d1e3ebf60dff6c".into(),
        chain_context: "0b91b8e4e44b2003a7c5e23ddadb5e14ef5345c0ebcb3ddcae07fa2f244cab76"
            .to_string(),
    })
}
```
