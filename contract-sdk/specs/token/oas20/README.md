# OAS-20 spec: Fungible Tokens

Specification for standardization of fungible token contracts within the Oasis contracts-sdk framework. Roughly based on [ERC20] and [CW20] specs.

[ERC20]: https://ethereum.org/en/developers/docs/standards/tokens/erc-20/
[CW20]: https://github.com/CosmWasm/cw-plus/blob/main/packages/cw20/README.md

## Requests

### Instantiate

```rust
#[cbor(rename = "instantiate")]
Instantiate(TokenInstantiation),

/// OAS20 token instantiation information.
#[derive(Debug, Default, Clone, PartialEq, Eq, cbor::Decode, cbor::Encode)]
pub struct TokenInstantiation {
    /// Name of the token.
    pub name: String,
    /// Token symbol.
    pub symbol: String,
    /// Number of decimals.
    pub decimals: u8,
    /// Initial balances of the token.
    #[cbor(optional, default, skip_serializing_if = "Vec::is_empty")]
    pub initial_balances: Vec<InitialBalance>,
    /// Information about minting in case the token supports minting.
    #[cbor(optional)]
    pub minting: Option<MintingInformation>,
}
```

Instantiates the OAS20 token contract.

### Transfer

```rust
#[cbor(rename = "transfer")]
Transfer { to: Address, amount: u128 },
```

Moves `amount` of tokens from the caller to the `to` account.

### Send

```rust
#[cbor(rename = "send")]
Send {
    to: InstanceId,
    amount: u128,
    data: cbor::Value,
},
```
Moves `amount` of tokens from the caller to the `to` contract and calls `ReceiveOas20` on the receiver contract. `to` is an instance ID of the receiving contract (not its address, to prevent sending to a non-contract address). The receiving contract should handle the `Receive { sender: Address, amount: u128, msg: cbor::Value }` call.

### Burn

```rust
#[cbor(rename = "burn")]
Burn { amount: u128 },
```

Removes `amount` of tokens from the caller and reduces the `total_supply` by `amount`.

### Allow

```rust
#[cbor(rename = "allow")]
Allow {
    beneficiary: Address,
    negative: bool,
    amount_change: u128,
},
```

Allow enables an account holder to set or decrease an allowance for a `beneficiary`.

### Withdraw

```rust
#[cbor(rename = "withdraw")]
Withdraw { from: Address, amount: u128 },
```

Withdraw enables a beneficiary to withdraw from the given account.

### Mint (optional)

```rust
#[cbor(rename = "mint")]
Mint { to: Address, amount: u128 },
```

Tokens supporting to be minted (potentially with a supply cap).

## Queries

### Token information

```rust
#[cbor(rename = "token_information")]
TokenInformation,
```

Returns the general token information:

```rust
#[cbor(rename = "token_information")]
TokenInformation { token_information: TokenInformation },

/// OAS20 token information.
#[derive(Debug, Default, Clone, PartialEq, Eq, cbor::Decode, cbor::Encode)]
pub struct TokenInformation {
    /// Name of the token.
    pub name: String,
    /// Token symbol.
    pub symbol: String,
    /// Number of decimals.
    pub decimals: u8,
    /// Total supply of the token.
    pub total_supply: u128,
    /// Information about minting in case the token supports minting.
    #[cbor(optional)]
    pub minting: Option<MintingInformation>,
}

/// Token minting information.
#[derive(Debug, Default, Clone, PartialEq, Eq, cbor::Decode, cbor::Encode)]
pub struct MintingInformation {
    /// Caller address which is allowed to mint new tokens.
    pub minter: Address,
    /// Cap on the total supply of the token.
    #[cbor(optional)]
    pub cap: Option<u128>,
}
```

### Balance

```rust
#[cbor(rename = "balance")]
Balance { address: Address },
```

Returns the balance of the account:

```rust
#[cbor(rename = "balance")]
Balance { balance: u128 },
```

### Allowance

```rust
#[cbor(rename = "allowance")]
Allowance { allower: Address, beneficiary: Address },
```

Returns the allowance set by `allower` to the `beneficiary`:

```rust
#[cbor(rename = "allowance")]
Allowance { allowance: u128 },
```

## Events

### OAS-20 Instantiated event

```rust
#[sdk_event(code = 1)]
Oas20Instantiated { token_information: TokenInformation },
```

Emitted on a successful OAS-20 token contract instantiation.

### OAS-20 Transferred event

```rust
#[sdk_event(code = 2)]
Oas20Transferred {
    from: Address,
    to: Address,
    amount: u128,
},
```

Emitted on a successful OAS-20 token transfer.

### OAS-20 Sent event

```rust
#[sdk_event(code = 3)]
Oas20Sent {
    from: Address,
    to: InstanceId,
    amount: u128,
},
```

Emitted on a successful OAS-20 token send.

### OAS-20 Burned event

```rust
#[sdk_event(code = 4)]
Oas20Burned { from: Address, amount: u128 },
```

Emitted on a successful OAS-20 token burn.

### OAS-20 Allowance changed event

```rust
#[sdk_event(code = 5)]
Oas20AllowanceChanged {
    owner: Address,
    beneficiary: Address,
    allowance: u128,
    negative: bool,
    amount_change: u128,
},
```

Emitted on a successful OAS-20 allow.

### OAS-20 Withdrew event

```rust
#[sdk_event(code = 6)]
Oas20Withdrew {
    from: Address,
    to: Address,
    amount: u128,
},
```

Emitted on a successful OAS-20 withdraw.

### OAS-20 Minted event

```rust
#[sdk_event(code = 7)]
Oas20Minted { to: Address, amount: u128 },
```

Emitted on a successful OAS-20 token mint.

## Errors

### Bad request

```rust
#[error("bad request")]
#[sdk_error(code = 1)]
BadRequest,
```

Error returned for requests not defined in this specification.

### Balance overflow

```rust
#[error("total supply overflow")]
#[sdk_error(code = 2)]
TotalSupplyOverflow,
```

Error returned in case total supply overflows maximum `u128` value during instantiation or minting.

### Zero amount

```rust
#[error("zero amount")]
#[sdk_error(code = 3)]
ZeroAmount,
```

Error returned in case zero amount is used in `transfer`, `send`, `burn` or `mint` actions.

### Insufficient funds

```rust
#[error("insufficient funds")]
#[sdk_error(code = 4)]
InsufficientFunds,
```

Error returned in case there are insufficient funds in the account to perform the action.

### Action forbidden

```rust
#[error("minting forbidden")]
#[sdk_error(code = 5)]
MintingForbidden,
```

Error returned in case a non authorized minter is trying to mint tokens.

### Mint over cap

```rust
#[error("mint over cap")]
#[sdk_error(code = 6)]
MintOverCap,
```

Error returned in case minting tokens would result in exceeding the configured minting cap.

### Same allower and beneficiary

```rust
#[error("allower and beneficiary same")]
#[sdk_error(code = 7)]
SameAllowerAndBeneficiary,
```

Error returned in case allower and beneficiary are the same in allowance transactions.

### Insufficient allowance

```rust
#[error("insufficient allowance")]
#[sdk_error(code = 8)]
InsufficientAllowance,
```

Error returned in case the withdrawer has insufficient allowance to withdraw.
