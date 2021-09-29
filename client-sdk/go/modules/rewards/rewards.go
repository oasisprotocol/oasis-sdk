package rewards

import (
	"context"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/client"
)

const (
	// Queries.
	methodParameters = "rewards.Parameters"
)

// V1 is the v1 rewards module interface.
type V1 interface {
	// Parameters queries the rewards module parameters.
	Parameters(ctx context.Context, round uint64) (*Parameters, error)
}

type v1 struct {
	rc client.RuntimeClient
}

// Implements V1.
func (a *v1) Parameters(ctx context.Context, round uint64) (*Parameters, error) {
	var params Parameters
	err := a.rc.Query(ctx, round, methodParameters, nil, &params)
	if err != nil {
		return nil, err
	}
	return &params, nil
}

// NewV1 generates a V1 client helper for the rewards module.
func NewV1(rc client.RuntimeClient) V1 {
	return &v1{rc: rc}
}
