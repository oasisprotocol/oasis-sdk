# Smart Contracts Module

## Open Questions

### General

- Cross-contract calls are currently proposed in an async fashion, where the
  contract can emit messages which are executed after the contract call
  complets, but within the same transaction (with optional gas limit). And the
  contract can also get the reply within the same transaction. This seems like
  easier to reason about and doesn't introduce reentrancy issues.

- Events.

### Confidentiality

- Probably want [oasis-core#3952] to ensure integrity of state roots (at least
  as long as we trust the consensus layer and discrepancy detection). This
  avoids the problem with compute nodes requesting executions using arbitrary
  state roots.

- A more strict check could be that the runtime would also require that the
  previous state root has a valid attestation from self or a whitelisted
  enclave (for upgrades). This would prevent even the consensus layer from
  tampering with state, but may make upgrades harder.

- State key derivation based on contract instance IDs. This should be fine as
  long as we trust the the above as an integrity check as otherwise this would
  allow the compute node to inject arbitrary code (given that key derivation is
  not based on the code hash).

- Transaction key pair derivation based on contract instance ID. Key rotation
  based on something like the epoch number would only make sense in case key
  managers only gave out keys for the epoch they see as current.

[oasis-core#3952]: https://github.com/oasisprotocol/oasis-core/issues/3952

### Code Storage

- This proposal just stores contract code in our MKVS. This could be suboptimal,
  but what is an alternative? Storing it in MKVS makes sure we get replication
  and checkpoints.

- Should we dedup by code hash?

## Contract Identifiers

There are two identifiers used throught the contracts module:

- Code identifier (`u64`) identifies specific contract code that has been
  uploaded. It may not yet be instantiated.
- Instance identifier (`u64`) identifies a specific deployed contract instance.

## State

- Next code identifier (`0x01`) stores a single `u64` used as a monotonic
  counter for code identifiers.
- Next instance identifier (`0x02`) stores a single `u64` used as a monotonic
  counter for instance identifiers.
- Code (`0x03 || <code-identifier>`) stores the `Code` structure that describes
  the uploaded code.
- Instance (`0x04 || <instance-identifier>`) stores the `Instance` structure
  that describes the deployed contract instance.
- Instance state (`0x05 || <instance-identifier> || ...`) is a per-instance
  key/value store that stores keys as `(H(key) || key) = value`.

  For now iterators are not supported.

## Types

```rust
/// Unique module name.
const MODULE_NAME: &str = "contracts";

/// Unique stored code identifier.
pub struct CodeId(u64);
/// Unique deployed code instance identifier.
pub struct InstanceId(u64);

pub enum Policy {
    Nobody,
    Address(Address),
    SelfOnly,
    Everyone,
}

/// Stored code.
pub struct Code {
    /// Unique code identifier.
    pub id: CodeId,

    /// Code hash.
    pub hash: Hash,

    /// Compiled code.
    pub code: Option<Vec<u8>>,

    /// Who is allowed to instantiate this code.
    pub instantiate_policy: Policy,
}

/// A deployed code instance.
pub struct Instance {
    pub id: InstanceId,
    pub code_id: CodeID,
    pub creator: Address,

    /// Who is allowed to call this instance.
    pub calls_policy: Policy,
    /// Who is allowed to upgrade this instance.
    pub upgrades_policy: Policy,
}

impl Instance {
    /// Account associated with the contract.
    pub fn account(&self) -> Address {
        Address::from_module_raw(MODULE_NAME, &self.id.to_be_bytes())
    }
}
```

## Parameters

```rust
pub struct Parameters {
    pub gas_costs: GasCosts,
}

pub struct GasCosts {
    pub tx_upload: u64,
    pub tx_upload_per_byte: u64,
    pub tx_instantiate: u64,
    pub tx_call: u64,
    pub tx_upgrade: u64,

    pub wasm_op: u64,

    // TODO: Costs of storage operations.
    // TODO: Cost of emitted messages.
    // TODO: Cost of queries.
}
```

## Transaction Methods

```rust
pub struct Upload {
    pub code: Vec<u8>,
    pub instantiate_policy: Policy,
}

pub type UploadResult = CodeId;

pub struct Instantiate {
    pub code_id: CodeId,
    pub calls_policy: Policy,
    pub upgrades_policy: Policy,
    pub data: Vec<u8>,
    pub tokens: Vec<token::BaseUnits>,
}

pub type InstantiateResult = InstanceId;

pub struct Call {
    pub id: InstanceId,
    pub data: Vec<u8>,
    pub tokens: Vec<token::BaseUnits>,
}

pub type CallResult = Vec<u8>;

pub struct Upgrade {
    pub id: InstanceId,
    pub code_id: CodeId,
    pub data: Vec<u8>,
    pub tokens: Vec<token::BaseUnits>,
}
```

## Query Methods

```rust
pub struct CodeQuery {
    pub id: CodeId,
}

pub type CodeQueryResult = Code;

pub struct InstanceQuery {
    pub id: InstanceId,
}

pub type InstanceQueryResult = Instance;

pub struct InstanceStorageQuery {
    pub id: InstanceId,
    pub key: Vec<u8>,
}

pub type InstanceStorageQueryResult = Option<Vec<u8>>;

pub enum PublicKeyKind {
    Transaction,
}

pub struct PublicKeyQuery {
    pub id: InstanceId,
    pub kind: PublicKeyKind,
}

pub struct PublicKeyQueryResult {
    /// Public key.
    pub key: PublicKey,

    /// Checksum of the key manager state.
    pub checksum: Vec<u8>,

    /// Sign(sk, (key || checksum)) from the key manager.
    pub signature: Signature,
}

pub struct CustomQuery {
    pub id: InstanceId,
    pub method: String,
    pub data: Vec<u8>,
}

pub type CustomQueryResult = Vec<u8>;
```

## Smart Contract Host Interface

### Smart Contract (exports)

```rust
/// Instantiate the smart contract.
///
/// Called during first instantiation. If it returns an error, the contract is
/// not instantiated.
pub fn instantiate(ctx: Context, request: &[u8]) -> Result<ExecutionResult, Error>;

/// Execute a given call.
pub fn execute(ctx: Context, request: &[u8]) -> Result<ExecutionResult, Error>;

/// Handle result from executing a call message.
pub fn message_call_result(ctx: Context, result: MessageCallResult) -> Result<ExecutionResult, Error>;

/// Execute the given read-only query.
///
/// # Confidentiality
///
/// This does not have access to any confidential state.
pub fn query(ctx: Context, query: &[u8]) -> Result<Vec<u8>, Error>;

/// Prepare upgrade of the smart contract.
///
/// Called with the old contract code before replacing it. If it returns an
/// error, the upgrade fails.
pub fn upgrade_prepare(ctx: Context, request: &[u8]) -> Result<Vec<u8>, Error>;

/// Upgrade the smart contract state.
///
/// Called with the new contract code after it has been replaced. This may
/// perform any state migrations.
///
/// If it returns an error, the upgrade fails.
pub fn upgrade(ctx: Context, request: &[u8]) -> Result<Vec<u8>, Error>;
```

Types:

```rust
/// Execution context.
///
/// Contains information that is useful on most invocations as it is always
/// included without requiring any explicit queries.
pub struct Context<'a> {
    /// Contract instance identifier.
    pub instance_id: InstanceId,
    /// Contract instance address.
    pub instance_address: Address,

    /// Authentication information about the transaction.
    pub tx_auth_info: Option<&'a AuthInfo>,
    /// Any tokens sent before execution.
    ///
    /// The tokens have already been deposited to the contract address before
    /// execution started.
    pub tx_deposited_tokens: Vec<token::BaseUnits>,
}

pub struct ExecutionResult {
    /// Raw data returned from the contract.
    pub data: Vec<u8>,

    /// Events emitted from the contract.
    pub events: Vec<Event>,

    /// Messages emitted from the contract.
    pub messages: Vec<Message>,
}

/// Messages can be emitted by contracts and are processed after the contract
/// execution completes.
pub struct Message {
    /// Call allows calling an arbitrary runtime method handler in a child
    /// context with an optional gas limit.
    ///
    /// The call is executed in the context of the smart contract as the
    /// caller within the same transaction.
    ///
    /// This can be used to call other smart contracts.
    Call {
        id: u64,
        reply: MessageCallReply,
        call: transaction::Call,
        max_gas: Option<u64>,
    },
}

/// Specifies when the caller (smart contract) wants to be notified of a reply.
pub enum MessageCallReply {
    Never,
    OnError,
    OnSuccess,
    Always,
}

/// Result of executing a call message.
pub struct MessageCallResult {
    /// Unique identifier used for correlating results.
    pub id: u64,

    /// The result of executing a call.
    pub result: transaction::CallResult,
}

// TODO: Event format and mapping to SDK events.
```

### Host (imports)

```rust
/// Fetch entry with given key.
pub fn get(key: &[u8]) -> Option<Vec<u8>>;

/// Update entry with given key to the given value.
pub fn insert(key: &[u8], value: &[u8]);

/// Remove entry with given key.
pub fn remove(key: &[u8]);

/// Query the host environment:
///
/// This includes the following queries:
///
/// * Current runtime block header.
/// * Account queries.
/// * Read only queries of other contracts' public state.
/// * Consensus layer state.
///
pub fn query(query: HostQueryRequest) -> HostQueryResponse;

// TODO: Common cryptographic functions for Ed25519 and Secp256k1.
```

Types:

```rust
pub enum HostQueryRequest {
    // Core queries.

    BlockInfo,

    // Account module queries.

    AccountInfo(Address),

    // Consensus layer queries.

    RandomBeacon,

    // TODO: Consensus accounts.
    // TODO: Consensus delegations.
}

pub enum HostQueryResponse {
    // Core runtime queries.

    BlockInfo {
        round: u64,
        epoch: u64,
        timestamp: u64,
        namespace: Namespace,
        chain_context: String,
    },

    // Account queries.

    AccountInfo {
        nonce: u64,
        balances: BTreeMap<token::Denomination, token::Quantity>,
    },

    // Consensus layer queries.

    RandomBeacon {
        epoch: u64,
        value: Vec<u8>,
    },
}
```
