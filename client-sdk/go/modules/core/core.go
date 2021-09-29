package core

import (
	"context"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/client"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

const (
	// Queries.
	methodEstimateGas = "core.EstimateGas"
)

// V1 is the v1 core module interface.
type V1 interface {
	// EstimateGas performs gas estimation for executing the given transaction.
	EstimateGas(ctx context.Context, round uint64, tx *types.Transaction) (uint64, error)
}

type v1 struct {
	rc client.RuntimeClient
}

// Implements V1.
func (a *v1) EstimateGas(ctx context.Context, round uint64, tx *types.Transaction) (uint64, error) {
	var gas uint64
	err := a.rc.Query(ctx, round, methodEstimateGas, tx, &gas)
	if err != nil {
		return 0, err
	}
	return gas, nil
}

// NewV1 generates a V1 client helper for the core module.
func NewV1(rc client.RuntimeClient) V1 {
	return &v1{rc: rc}
}
