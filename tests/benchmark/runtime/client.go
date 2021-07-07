package runtime

import (
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/client"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

const (
	methodAccoutsMint      = "benchmarks.accounts.Mint"
	methodAccountsTransfer = "benchmarks.accounts.Transfer"
)

type V1 interface {
	AccountsMint(amount types.BaseUnits) *client.TransactionBuilder

	AccountsTransfer(to types.Address, amount types.BaseUnits) *client.TransactionBuilder
}

type v1 struct {
	rc client.RuntimeClient
}

// Implements V1.
func (a *v1) AccountsMint(amount types.BaseUnits) *client.TransactionBuilder {
	return client.NewTransactionBuilder(a.rc, methodAccoutsMint, &AccountsMint{
		Amount: amount,
	})
}

// Implements V1.
func (a *v1) AccountsTransfer(to types.Address, amount types.BaseUnits) *client.TransactionBuilder {
	return client.NewTransactionBuilder(a.rc, methodAccountsTransfer, &AccountsTransfer{
		Amount: amount,
		To:     to,
	})
}

func NewV1(rc client.RuntimeClient) V1 {
	return &v1{rc: rc}
}
