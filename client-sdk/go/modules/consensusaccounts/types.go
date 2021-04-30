package consensusaccounts

import "github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"

// Deposit are the arguments for consensus.Deposit method.
type Deposit struct {
	Amount types.BaseUnits `json:"amount"`
}

// Withdraw are the arguments for consensus.Deposit method.
type Withdraw struct {
	Amount types.BaseUnits `json:"amount"`
}

// BalanceQuery are the arguments for consensus.Balance method.
type BalanceQuery struct {
	Address types.Address `json:"address"`
}

// AccountBalance is the consensus balance in an account.
type AccountBalance struct {
	Balance types.Quantity `json:"balance"`
}

// AccountQuery are the arguments for consensus.Account method.
type AccountQuery struct {
	Address types.Address `json:"address"`
}
