package accounts

import (
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

// NonceQuery are the arguments for the accounts.Nonce query.
type NonceQuery struct {
	Address types.Address `json:"address"`
}

// BalancesQuery are the arguments for the accounts.Balances query.
type BalancesQuery struct {
	Address types.Address `json:"address"`
}

// AccountBalances are the balances in an account.
type AccountBalances struct {
	Balances map[types.Denomination]types.Quantity `json:"balances"`
}
