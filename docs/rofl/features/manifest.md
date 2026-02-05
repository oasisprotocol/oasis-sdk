---
toc_max_heading_level: 4
---

# `rofl.yaml` Manifest File

## Metadata {#metadata}

The following fields are valid in your yaml root:

- `name`: A short, human-readabe name for your app. e.g. `my-app`
- `version`: ROFL version. e.g. `0.1.1`
- `repository`: A path to the git repository. e.g.
  `https://github.com/user/my-app`
- `author`: The author name and their e-mail address. e.g.
  `John Doe <john@doe.com>`
- `license`: The ROFL license in [SPDX] format. e.g. `Apache-2.0`
- `tee`: The Trusted Execution Environment type. Valid options are `tdx`
  (default) or `sgx`
- `kind`: The ROFL "flavor". Valid options for TDX TEE are `containers`
  (default) or `raw`. The only valid option for SGX TEE is `raw`

[SPDX]: https://spdx.org/licenses/

## App Resources (`resources`) {#resources}

Each containerized app running in ROFL must define what kind of resources it
needs for its execution. This includes the number of assigned vCPUs, amount of
memory, storage requirements, GPUs, etc.

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
the app and will require the policy to be updated!

:::

### Memory (`memory`) {#resources-memory}

The amount of memory is specified in megabytes. By default the this value is
initialized to `512`.

### vCPU Count (`cpus`) {#resources-cpu}

The number of vCPUs allocated to the VM. By default this value is initialized to
`1`.

### Storage (`storage`) {#resources-storage}

Each app running in ROFL can request different storage options, depending on its
use case. The storage kind is specified in the `kind` field with the following
values currently supported:

- `disk-persistent`: Provisions a persistent disk of the given size. The disk is
  encrypted and authenticated using a key derived by the decentralized on-chain
  key management system after successful attestation.

- `disk-ephemeral`: Provisions an ephemeral disk of the given size. The disk is
  encrypted and authenticated using an ephemeral key randomly generated on each
  boot.

- `ram`: Provisions an ephemeral filesystem entirely contained in encrypted
  memory.

- `none`: Does not provision any kind of storage. Specifying this option will
  not work for containerized apps.

The `size` argument defines the amount of storage to provision in megabytes.

## Deployments (`deployments`) {#deployments}

This section contains ROFL deployments on specific networks.

### `<deployment_name>`

User-defined deployment name.

#### `policy`

Contains the policy under which the app will be allowed to spin up:

- `quotes`: A TEE-specific policy requirements such as the TCB validity
  period, and the minimum TCB-R number which indicates what security updates
  must be applied to the given platform.
- `enclaves`: Allowed enclave IDs for running this app.
- `endorsements`: A list of conditions that define who can run this app.
  - `- any: {}`: Any node is allowed to run the app.
  - `- node: <node_id>`: Node with a specific node ID is allowed to run the app.
  - `- provider: <address>`: Nodes belonging to the specified ROFL provider
    are allowed to run the app.
  - `- provider_instance_admin: <address>`: Machines having the specified admin
    are allowed to run the app.

  You can also nest conditions with `and` and `or` operators. For example:

  ```yaml title="policy.yaml"
  endorsements:
    - and:
      - provider: oasis1qp2ens0hsp7gh23wajxa4hpetkdek3swyyulyrmz
      - or:
        - provider_instance_admin: oasis1qrk58a6j2qn065m6p06jgjyt032f7qucy5wqeqpt
        - provider_instance_admin: oasis1qqcd0qyda6gtwdrfcqawv3s8cr2kupzw9v967au6
  ```

  In the example the app will only run on a specified provider and on machines
  owned by either of the two admin addresses.

- `fees: <fee_policy>`: Who pays for the registration and other fees:
  - `endorsing_node`: The node running the app pays the fees.
  - `instance`: The app instance pays the fees.

#### `machines`

Contains machines which the specific app deployment lives on. A new `default`
machine is created during [`oasis rofl deploy`] if none exists yet. Otherwise,
the existing machine is considered for redeployment of the app.

- `<machine_name>`: User-defined machine name.
  - `provider: <provider_address>`: Oasis native address of the ROFL provider
    hosting the machine.
  - `offer: <offer_name>`: The name of the offer used.
  - `id: <machine_id>`: Unique ID of the machine per provider.
  - `permissions` (optional): ROFL scheduler-specific permissions.
    - `log.view`: List of Oasis native addresses that can access machine logs

[`oasis rofl deploy`]: https://github.com/oasisprotocol/cli/blob/master/docs/rofl.md#deploy
