// Package runtime implements the benchmarking runtime client.
package runtime

import (
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

// AccoutsMint are the arguments for the benchmarking.accounts.Mint transaction.
type AccountsMint struct {
	Amount types.BaseUnits `json:"amount"`
}

// AccoutsTransfer are the arguments for the benchmarking.accounts.Transfer transaction.
type AccountsTransfer struct {
	Amount types.BaseUnits `json:"amount"`

	To types.Address `json:"to"`
}
