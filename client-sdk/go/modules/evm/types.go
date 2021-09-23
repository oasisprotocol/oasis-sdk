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

// PeekStorageQuery queries the EVM storage.
type PeekStorageQuery struct {
	Address []byte `json:"address"`
	Index   []byte `json:"index"`
}

// PeekCodeQuery queries the EVM code storage.
type PeekCodeQuery struct {
	Address []byte `json:"address"`
}
