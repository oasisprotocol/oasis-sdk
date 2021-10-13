package evm

import (
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

// The types in this file must match the types from the evm module types
// in runtime-sdk/modules/evm/src/types.rs.

// Create is an EVM CREATE transaction.
type Create struct {
	Value    []byte `json:"value"`
	InitCode []byte `json:"init_code"`
}

// Call is an EVM CALL transaction.
type Call struct {
	Address []byte `json:"address"`
	Value   []byte `json:"value"`
	Data    []byte `json:"data"`
}

// Deposit is a transaction that deposits tokens into an EVM account.
type Deposit struct {
	To     []byte          `json:"to"`
	Amount types.BaseUnits `json:"amount"`
}

// Withdraw is a transaction that withdraws tokens from an EVM account.
type Withdraw struct {
	To     types.Address   `json:"to"`
	Amount types.BaseUnits `json:"amount"`
}

// StorageQuery queries the EVM storage.
type StorageQuery struct {
	Address []byte `json:"address"`
	Index   []byte `json:"index"`
}

// CodeQuery queries the EVM code storage.
type CodeQuery struct {
	Address []byte `json:"address"`
}

// BalanceQuery queries the EVM account balance.
type BalanceQuery struct {
	Address []byte `json:"address"`
}

// SimulateCallQuery simulates an EVM CALL.
type SimulateCallQuery struct {
	GasPrice []byte `json:"gas_price"`
	GasLimit uint64 `json:"gas_limit"`
	Caller   []byte `json:"caller"`
	Address  []byte `json:"address"`
	Value    []byte `json:"value"`
	Data     []byte `json:"data"`
}
