# `rofl.yaml` Manifest File

## Metadata {#metadata}

The following fields are valid in your yaml root:

- `name`: A short, human-readabe name for your ROFL app. e.g. `my-rofl-app`
- `version`: ROFL version. e.g. `0.1.1`
- `repository`: A path to the git repository. e.g.
  `https://github.com/user/my-rofl-app`
- `author`: The author name and their e-mail address. e.g.
  `John Doe <john@doe.com>`
- `license`: The ROFL license in [SPDX] format. e.g. `Apache-2.0`
- `tee`: The Trusted Execution Environment type. Valid options are `tdx`
  (default) or `sgx`
- `kind`: The ROFL "flavor". Valid options for TDX TEE are `containers`
  (default) or `raw`. The only valid option for SGX TEE is `raw`

[SPDX]: https://spdx.org/licenses/

## App Resources (`resources`) {#resources}

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

### Memory (`memory`) {#resources-memory}

The amount of memory is specified in megabytes. By default the this value is
initialized to `512`.

### vCPU Count (`cpus`) {#resources-cpu}

The number of vCPUs allocated to the VM. By default this value is initialized to
`1`.

### Storage (`storage`) {#resources-storage}

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

## Deployments (`deployments`)

This section contains ROFL deployments on specific networks.

### `<deployment name>`

#### `policy`

Contains the policy under which the ROFL app will be allowed to spin up:

- `quotes`: defines a TEE-specific policy requirements such as the TCB validity
  period, and the minimum TCB-R number which indicates what security updates
  must be applied to the given platform.
- `enclaves`: defines the allowed enclave IDs for running this ROFL app.
- `endorsements`: a list of nodes which can run this ROFL app.
  - `- any: {}`: any node is allowed to run this ROFL app.
  - `- node: <node_id>`: node with a specific node ID is allowed to run this
    ROFL app. You can provide multiple `node` entries to allow the execution by
    any of the listed nodes.
- `fees: <fee_policy>`: who pays for the registration and other fees:
  - `endorsing_node`: the node running ROFL app pays the fees.
  - `instance`: ROFL app instance pays the fees.
