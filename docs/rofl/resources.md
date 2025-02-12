# Resources

Each containerized ROFL app must define what kind of resources it needs for its
execution. This includes the number of assigned vCPUs, amount of memory, storage
requirements, GPUs, etc.

Resources are specified in the app manifest file under the `resources` section
as follows:

```yaml
resources:
  memory: 512
  cpus: 1
  storage:
    kind: disk-persistent
    size: 512
```

This chapter describes the set of supported resources.

:::warning

Changing the requested resources will result in a different enclave identity of
the ROFL app and will require the policy to be updated!

:::

## Memory (`memory`)

The amount of memory is specified in megabytes. By default the this value is
initialized to `512`.

## vCPU Count (`cpus`)

The number of vCPUs allocated to the VM. By default this value is initialized to
`1`.

## Storage (`storage`)

Each ROFL app can request different storage options, depending on its use case.
The storage kind is specified in the `kind` field with the following values
currently supported:

- `disk-persistent` provisions a persistent disk of the given size. The disk is
  encrypted and authenticated using a key derived by the decentralized on-chain
  key management system after successful attestation.

- `disk-ephemeral` provisions an ephemeral disk of the given size. The disk is
  encrypted and authenticated using an ephemeral key randomly generated on each
  boot.

- `ram` provisions an ephemeral filesystem entirely contained in encrypted
  memory.

- `none` does not provision any kind of storage. Specifying this option will not
  work for containerized apps.

The `size` argument defines the amount of storage to provision in megabytes.
