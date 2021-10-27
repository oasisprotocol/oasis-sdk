package core

import (
	"context"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/client"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

const (
	// Queries.
	methodEstimateGas = "core.EstimateGas"
	methodMinGasPrice = "core.MinGasPrice"
)

// V1 is the v1 core module interface.
type V1 interface {
	// EstimateGas performs gas estimation for executing the given transaction.
	EstimateGas(ctx context.Context, round uint64, tx *types.Transaction) (uint64, error)

	// EstimateGasForCaller performs gas estimation for executing the given transaction as if the
	// caller specified by address had executed it.
	EstimateGasForCaller(ctx context.Context, round uint64, caller types.Address, tx *types.Transaction) (uint64, error)

	// MinGasPrice returns the minimum gas price.
	MinGasPrice(ctx context.Context) (map[types.Denomination]types.Quantity, error)
}

type v1 struct {
	rc client.RuntimeClient
}

// Implements V1.
func (a *v1) EstimateGas(ctx context.Context, round uint64, tx *types.Transaction) (uint64, error) {
	var gas uint64
	err := a.rc.Query(ctx, round, methodEstimateGas, EstimateGasQuery{Tx: tx}, &gas)
	if err != nil {
		return 0, err
	}
	return gas, nil
}

// Implements V1.
func (a *v1) EstimateGasForCaller(ctx context.Context, round uint64, caller types.Address, tx *types.Transaction) (uint64, error) {
	var gas uint64
	args := EstimateGasQuery{
		Caller: &caller,
		Tx:     tx,
	}
	err := a.rc.Query(ctx, round, methodEstimateGas, args, &gas)
	if err != nil {
		return 0, err
	}
	return gas, nil
}

// Implements V1.
func (a *v1) MinGasPrice(ctx context.Context) (map[types.Denomination]types.Quantity, error) {
	var mgp map[types.Denomination]types.Quantity
	err := a.rc.Query(ctx, client.RoundLatest, methodMinGasPrice, nil, &mgp)
	if err != nil {
		return nil, err
	}
	return mgp, nil
}

// NewV1 generates a V1 client helper for the core module.
func NewV1(rc client.RuntimeClient) V1 {
	return &v1{rc: rc}
}
