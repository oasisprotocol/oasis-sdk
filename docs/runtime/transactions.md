# Runtime Transactions

<!-- The (incomplete) list below was composed manually in March 2022 as a
     reference guide proof-of-concept. Feel free to update it as long as there
     is no automated way of doing it. -->

This section describes the format of all supported runtime methods and queries
with references to Go, Rust and TypeScript bindings in Oasis SDK.

## Methods

### accounts.Transfer
[[Go][go-accounts.Transfer]|[Rust][rust-accounts.Transfer]|[TypeScript][ts-accounts.Transfer]]

Transfer call.

#### Parameters [[Go][go-params-accounts.Transfer]|[Rust][rust-params-accounts.Transfer]|[TypeScript][ts-params-accounts.Transfer]]

- `to: Address`
- `amount: token::BaseUnits`

[go-accounts.Transfer]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/client-sdk/go/modules/accounts/accounts.go#L55-L61
[rust-accounts.Transfer]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/runtime-sdk/src/modules/accounts/mod.rs#L690-L703
[ts-accounts.Transfer]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/client-sdk/ts-web/rt/src/accounts.ts#L35-L37
[go-params-accounts.Transfer]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/client-sdk/go/modules/accounts/types.go#L7-L11
[rust-params-accounts.Transfer]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/runtime-sdk/src/modules/accounts/types.rs#L10-L13
[ts-params-accounts.Transfer]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/client-sdk/ts-web/rt/src/types.ts#L132-L138

### contracts.Call
[[Go][go-contracts.Call]|[Rust][rust-contracts.Call]|[TypeScript][ts-contracts.Call]]

Contract call.

#### Parameters [[Go][go-params-contracts.Call]|[Rust][rust-params-contracts.Call]|[TypeScript][ts-params-contracts.Call]]

- `id: InstanceId`

  Instance identifier.

- `data: Vec<u8>`

  Call arguments.

- `tokens: Vec<token::BaseUnits>`
  
  Tokens that should be sent to the contract as part of the call.

#### Result [[Go][go-result-contracts.Call]|[Rust][rust-result-contracts.Call]|[TypeScript][ts-result-contracts.Call]]

- `Vec<u8>`

[go-contracts.Call]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/client-sdk/go/modules/contracts/contracts.go#L144-L147
[rust-contracts.Call]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/runtime-sdk/modules/contracts/src/lib.rs#L486-L528
[ts-contracts.Call]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/client-sdk/ts-web/rt/src/contracts.ts#L58-L60
[go-params-contracts.Call]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/client-sdk/go/modules/contracts/types.go#L109-L117
[rust-params-contracts.Call]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/runtime-sdk/modules/contracts/src/types.rs#L140-L149
[ts-params-contracts.Call]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/client-sdk/ts-web/rt/src/types.ts#L263-L243
[go-result-contracts.Call]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/client-sdk/go/modules/contracts/types.go#L119-L120
[rust-result-contracts.Call]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/runtime-sdk/modules/contracts/src/types.rs#L151-L154
[ts-result-contracts.Call]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/client-sdk/ts-web/rt/src/types.ts#L245-L252

### contracts.Instantiate
[[Go][go-contracts.Instantiate]|[Rust][rust-contracts.Instantiate]|[TypeScript][ts-contracts.Instantiate]]

Instantiate call.

#### Parameters [[Go][go-params-contracts.Instantiate]|[Rust][rust-params-contracts.Instantiate]|[TypeScript][ts-params-contracts.Instantiate]]

- `code_id: CodeId`

  Identifier of code used by the instance.
  
- `upgrades_policy: Policy`

  Who is allowed to upgrade this instance.

- `data: Vec<u8>`
  
  Arguments to contract's instantiation function.

- `tokens: Vec<token::BaseUnits>`
  
  Tokens that should be sent to the contract as part of the instantiate call.

#### Result [[Go][go-result-contracts.Instantiate]|[Rust][rust-result-contracts.Instantiate]|[TypeScript][ts-result-contracts.Instantiate]]

- `id: InstanceId`

  Assigned instance identifier.

[go-contracts.Instantiate]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/client-sdk/go/modules/contracts/contracts.go#L130-L133
[rust-contracts.Instantiate]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/runtime-sdk/modules/contracts/src/lib.rs#L424-L483
[ts-contracts.Instantiate]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/client-sdk/ts-web/rt/src/contracts.ts#L53-L57
[go-params-contracts.Instantiate]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/client-sdk/go/modules/contracts/types.go#L91-L101
[rust-params-contracts.Instantiate]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/runtime-sdk/modules/contracts/src/types.rs#L115-L129
[ts-params-contracts.Instantiate]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/client-sdk/ts-web/rt/src/types.ts#L502-L522
[go-result-contracts.Instantiate]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/client-sdk/go/modules/contracts/contracts.go#L103-L107
[rust-result-contracts.Instantiate]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/runtime-sdk/modules/contracts/src/types.rs#L131-L136
[ts-result-contracts.Instantiate]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/client-sdk/ts-web/rt/src/types.ts#L524-L532

### contracts.Upgrade
[[Go][go-contracts.Upgrade]|[Rust][rust-contracts.Upgrade]|[TypeScript][ts-contracts.Upgrade]]

Upgrade call.

#### Parameters [[Go][go-params-contracts.Upgrade]|[Rust][rust-params-contracts.Upgrade]|[TypeScript][ts-params-contracts.Upgrade]]

- `id: InstanceId`

  Instance identifier.

- `code_id: CodeId`

  Updated code identifier.

- `data: Vec<u8>`
  
  Arguments to contract's upgrade function.

- `tokens: Vec<token::BaseUnits>`

  Tokens that should be sent to the contract as part of the call.

[go-contracts.Upgrade]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/client-sdk/go/modules/contracts/contracts.go#L159-L162
[rust-contracts.Upgrade]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/runtime-sdk/modules/contracts/src/lib.rs#L531-L597
[ts-contracts.Upgrade]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/client-sdk/ts-web/rt/src/contracts.ts#L61-L63
[go-params-contracts.Upgrade]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/client-sdk/go/modules/contracts/types.go#L122-L132
[rust-params-contracts.Upgrade]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/runtime-sdk/modules/contracts/src/types.rs#L156-L170
[ts-params-contracts.Upgrade]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/client-sdk/ts-web/rt/src/types.ts#L552-L572

### contracts.Upload
[[Go][go-contracts.Upload]|[Rust][rust-contracts.Upload]|[TypeScript][ts-contracts.Upload]]

Upload call.

#### Parameters [[Go][go-params-contracts.Upload]|[Rust][rust-params-contracts.Upload]|[TypeScript][ts-params-contracts.Upload]]

- `abi: ABI`
  
  ABI
  
- `instantiate_policy: Policy`
  
  Who is allowed to instantiate this code.
  
- `code: Vec<u8>`
  
  Compiled contract code.

#### Result [[Go][go-result-contracts.Upload]|[Rust][rust-result-contracts.Upload]|[TypeScript][ts-result-contracts.Upload]]

- `id: CodeId`
  
  Assigned code identifier.

[go-contracts.Upload]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/client-sdk/go/modules/contracts/contracts.go#L111-L118
[rust-contracts.Upload]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/runtime-sdk/modules/contracts/src/lib.rs#L332-L421
[ts-contracts.Upload]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/client-sdk/ts-web/rt/src/contracts.ts#L50-L52
[go-params-contracts.Upload]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/client-sdk/go/modules/contracts/types.go#L73-L83
[rust-params-contracts.Upload]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/runtime-sdk/modules/contracts/src/types.rs#L95-L106
[ts-params-contracts.Upload]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/client-sdk/ts-web/rt/src/types.ts#L474-L490
[go-result-contracts.Upload]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/client-sdk/go/modules/contracts/types.go#L85-L89
[rust-result-contracts.Upload]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/runtime-sdk/modules/contracts/src/types.rs#L108-L113
[ts-result-contracts.Upload]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/client-sdk/ts-web/rt/src/types.ts#L492-L500

### evm.Call
[[Go][go-evm.Call]|[Rust][rust-evm.Call]|[TypeScript][ts-evm.Call]]

Transaction body for calling an EVM contract.

#### Parameters [[Go][go-params-evm.Call]|[Rust][rust-params-evm.Call]|[TypeScript][ts-params-evm.Call]]

- `address: H160`
- `value: U256`
- `data: Vec<u8>`

[go-evm.Call]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/client-sdk/go/modules/evm/evm.go#L73-L80
[rust-evm.Call]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/runtime-sdk/modules/evm/src/lib.rs#L599-L601
[ts-evm.Call]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/client-sdk/ts-web/rt/src/evm.ts#L40-L42
[go-params-evm.Call]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/client-sdk/go/modules/evm/types.go#L12-L17
[rust-params-evm.Call]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/runtime-sdk/modules/evm/src/types.rs#L10-L16
[ts-params-evm.Call]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/client-sdk/ts-web/rt/src/types.ts#L402-L409

### evm.Create
[[Go][go-evm.Create]|[Rust][rust-evm.Create]|[TypeScript][ts-evm.Create]]

Transaction body for creating an EVM contract.

#### Parameters [[Go][go-params-evm.Create]|[Rust][rust-params-evm.Create]|[TypeScript][ts-params-evm.Create]]

- `value: U256`
- `init_code: Vec<u8>`

[go-evm.Create]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/client-sdk/go/modules/evm/evm.go#L65-L71
[rust-evm.Create]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/runtime-sdk/modules/evm/src/lib.rs#L594-L596
[ts-evm.Create]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/client-sdk/ts-web/rt/src/evm.ts#L36-L38
[go-params-evm.Create]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/client-sdk/go/modules/evm/types.go#L6-L10
[rust-params-evm.Create]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/runtime-sdk/modules/evm/src/types.rs#L3-L8
[ts-params-evm.Create]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/client-sdk/ts-web/rt/src/types.ts#L394-L400

### consensus.Deposit
[[Go][go-consensus.Deposit]|[Rust][rust-consensus.Deposit]|[TypeScript][ts-consensus.Deposit]]

Deposit into runtime call.
Transfer from consensus staking to an account in this runtime.
The transaction signer has a consensus layer allowance benefiting this runtime's staking
address. The `to` address runtime account gets the tokens.

#### Parameters [[Go][go-params-consensus.Deposit]|[Rust][rust-params-consensus.Deposit]|[TypeScript][ts-params-consensus.Deposit]]

- `to: Option<Address>`
- `amount: token::BaseUnits`

[go-consensus.Deposit]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/client-sdk/go/modules/consensusaccounts/consensus_accounts.go#L52-L58
[rust-consensus.Deposit]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/runtime-sdk/src/modules/consensus_accounts/mod.rs#L230-L240
[ts-consensus.Deposit]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/client-sdk/ts-web/rt/src/consensus_accounts.ts#L31-L33
[go-params-consensus.Deposit]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/client-sdk/go/modules/consensusaccounts/types.go#L5-L9
[rust-params-consensus.Deposit]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/runtime-sdk/src/modules/consensus_accounts/types.rs#L4-L13
[ts-params-consensus.Deposit]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/client-sdk/ts-web/rt/src/types.ts#L336-L342

### consensus.Withdraw
[[Go][go-consensus.Withdraw]|[Rust][rust-consensus.Withdraw]|[TypeScript][ts-consensus.Withdraw]]

Withdraw from runtime call.
Transfer from an account in this runtime to consensus staking.
The `to` address consensus staking account gets the tokens.

#### Parameters [[Go][go-params-consensus.Withdraw]|[Rust][rust-params-consensus.Withdraw]|[TypeScript][ts-params-consensus.Withdraw]]

- `to: Option<Address>`
- `amount: token::BaseUnits`

[go-consensus.Withdraw]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/client-sdk/go/modules/consensusaccounts/consensus_accounts.go#L60-L66
[rust-consensus.Withdraw]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/runtime-sdk/src/modules/consensus_accounts/mod.rs#L244-L260
[ts-consensus.Withdraw]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/client-sdk/ts-web/rt/src/consensus_accounts.ts#L35-L37
[go-params-consensus.Withdraw]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/client-sdk/go/modules/consensusaccounts/types.go#L11-L15
[rust-params-consensus.Withdraw]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/runtime-sdk/src/modules/consensus_accounts/types.rs#L15-L23
[ts-params-consensus.Withdraw]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/client-sdk/ts-web/rt/src/types.ts#L344-L350

## Queries

### accounts.Addresses
[[Go][go-accounts.Addresses]|[Rust][rust-accounts.Addresses]|[TypeScript][ts-accounts.Addresses]]

Arguments for the Addresses query.

#### Parameters [[Go][go-params-accounts.Addresses]|[Rust][rust-params-accounts.Addresses]|[TypeScript][ts-params-accounts.Addresses]]

- `denomination: token::Denomination`

[go-accounts.Addresses]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/client-sdk/go/modules/accounts/accounts.go#L94-L101
[rust-accounts.Addresses]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/runtime-sdk/src/modules/accounts/mod.rs#L711-L720
[ts-accounts.Addresses]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/client-sdk/ts-web/rt/src/accounts.ts#L49-L51
[go-params-accounts.Addresses]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/client-sdk/go/modules/accounts/types.go#L28-L31
[rust-params-accounts.Addresses]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/runtime-sdk/src/modules/accounts/types.rs#L30-L34
[ts-params-accounts.Addresses]: https://github.com/oasisprotocol/oasis-sdk/blob/656b0a21527149c690c3daf3ce25becea6e9bad3/client-sdk/ts-web/rt/src/types.ts#L111-L116

### accounts.Balances 

### accounts.DenominationInfo

### accounts.Nonce

### contracts.Code

### contracts.Custom

### contracts.Instance

### contracts.InstanceStorage

### contracts.PublicKey

### consensus.Account

### consensus.Balance

### core.CallDataPublicKey

### core.CheckInvariants

### core.EstimateGas

### core.MinGasPrice

### core.RuntimeInfo

### evm.Balance

### evm.Code

### evm.SimulateCall

### evm.Storage
