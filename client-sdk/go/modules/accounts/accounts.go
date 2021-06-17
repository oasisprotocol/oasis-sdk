package accounts

import (
	"context"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/client"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

const (
	// Callable methods.
	methodTransfer = "accounts.Transfer"

	// Queries.
	methodNonce    = "accounts.Nonce"
	methodBalances = "accounts.Balances"
)

// V1 is the v1 accounts module interface.
type V1 interface {
	// Transfer generates an accounts.Transfer transaction.
	Transfer(to types.Address, amount types.BaseUnits) *client.TransactionBuilder

	// Nonce queries the given account's nonce.
	Nonce(ctx context.Context, round uint64, address types.Address) (uint64, error)

	// Balances queries the given account's balances.
	Balances(ctx context.Context, round uint64, address types.Address) (*AccountBalances, error)
}

type v1 struct {
	rc client.RuntimeClient
}

// Implements V1.
func (a *v1) Transfer(to types.Address, amount types.BaseUnits) *client.TransactionBuilder {
	return client.NewTransactionBuilder(a.rc, methodTransfer, &Transfer{
		To:     to,
		Amount: amount,
	})
}

// Implements V1.
func (a *v1) Nonce(ctx context.Context, round uint64, address types.Address) (uint64, error) {
	var nonce uint64
	err := a.rc.Query(ctx, round, methodNonce, &NonceQuery{Address: address}, &nonce)
	if err != nil {
		return 0, err
	}
	return nonce, nil
}

// Implements V1.
func (a *v1) Balances(ctx context.Context, round uint64, address types.Address) (*AccountBalances, error) {
	var balances AccountBalances
	err := a.rc.Query(ctx, round, methodBalances, &BalancesQuery{Address: address}, &balances)
	if err != nil {
		return nil, err
	}
	return &balances, nil
}

// NewV1 generates a V1 client helper for the accounts module.
func NewV1(rc client.RuntimeClient) V1 {
	return &v1{rc: rc}
}

// NewTransferTx generates a new accounts.Transfer transaction.
func NewTransferTx(fee *types.Fee, body *Transfer) *types.Transaction {
	return types.NewTransaction(fee, methodTransfer, body)
}
