package accounts

import (
	"context"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/client"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

const (
	methodNonce    = "accounts.Nonce"
	methodBalances = "accounts.Balances"
)

type V1 interface {
	Nonce(ctx context.Context, round uint64, address types.Address) (uint64, error)

	Balances(ctx context.Context, round uint64, address types.Address) (*AccountBalances, error)
}

type v1 struct {
	rc client.RuntimeClient
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

func NewV1(rc client.RuntimeClient) V1 {
	return &v1{rc: rc}
}
