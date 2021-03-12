package client

import (
	"context"
	"fmt"

	"google.golang.org/grpc"

	"github.com/oasisprotocol/oasis-core/go/common"
	"github.com/oasisprotocol/oasis-core/go/common/cbor"
	"github.com/oasisprotocol/oasis-core/go/common/crypto/hash"
	"github.com/oasisprotocol/oasis-core/go/common/pubsub"
	consensus "github.com/oasisprotocol/oasis-core/go/consensus/api"
	roothash "github.com/oasisprotocol/oasis-core/go/roothash/api"
	coreClient "github.com/oasisprotocol/oasis-core/go/runtime/client/api"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

// RoundLatest is a special round number always referring to the latest round.
const RoundLatest = coreClient.RoundLatest

// RuntimeClient is a client interface for runtimes based on the Oasis Runtime SDK.
type RuntimeClient interface {
	// GetInfo returns information about the runtime.
	GetInfo(ctx context.Context) (*types.RuntimeInfo, error)

	SubmitTx(ctx context.Context, tx *types.UnverifiedTransaction) (cbor.RawMessage, error)

	// GetTransactions(ctx context.Context, round uint64) ([][]byte, error)

	// GetEvents returns all events emitted in a given block.
	GetEvents(ctx context.Context, round uint64) ([]*coreClient.Event, error)

	// WatchBlocks subscribes to blocks for a specific runtimes.
	WatchBlocks(ctx context.Context) (<-chan *roothash.AnnotatedBlock, pubsub.ClosableSubscription, error)

	Query(ctx context.Context, round uint64, method string, args, rsp interface{}) error
}

// Event is an event emitted by a runtime in the form of a runtime transaction tag.
//
// Key and value semantics are runtime-dependent.
// TODO: More high-level wrapper for SDK events.
type Event struct {
	Module string
	Code   uint32
	TxHash hash.Hash
	Value  cbor.RawMessage
}

type runtimeClient struct {
	cs        consensus.ClientBackend
	cc        coreClient.RuntimeClient
	runtimeID common.Namespace
}

// Implements RuntimeClient.
func (rc *runtimeClient) GetInfo(ctx context.Context) (*types.RuntimeInfo, error) {
	chainCtx, err := rc.cs.GetChainContext(ctx)
	if err != nil {
		return nil, fmt.Errorf("failed to fetch consensus layer chain context: %w", err)
	}

	return &types.RuntimeInfo{
		ID:           rc.runtimeID,
		ChainContext: signature.DeriveChainContext(rc.runtimeID, chainCtx),
	}, nil
}

// Implements RuntimeClient.
func (rc *runtimeClient) SubmitTx(ctx context.Context, tx *types.UnverifiedTransaction) (cbor.RawMessage, error) {
	raw, err := rc.cc.SubmitTx(ctx, &coreClient.SubmitTxRequest{
		RuntimeID: rc.runtimeID,
		Data:      cbor.Marshal(tx),
	})
	if err != nil {
		return nil, err
	}

	var result types.CallResult
	if err = cbor.Unmarshal(raw, &result); err != nil {
		return nil, fmt.Errorf("failed to unmarshal call result: %w", err)
	}
	if !result.IsSuccess() {
		return nil, result.Failed
	}
	return result.Ok, nil
}

// Implements RuntimeClient.
func (rc *runtimeClient) WatchBlocks(ctx context.Context) (<-chan *roothash.AnnotatedBlock, pubsub.ClosableSubscription, error) {
	return rc.cc.WatchBlocks(ctx, rc.runtimeID)
}

// Implements RuntimeClient.
func (rc *runtimeClient) GetEvents(ctx context.Context, round uint64) ([]*coreClient.Event, error) {
	return rc.cc.GetEvents(ctx, &coreClient.GetEventsRequest{
		RuntimeID: rc.runtimeID,
		Round:     round,
	})
}

// Implements RuntimeClient.
func (rc *runtimeClient) Query(ctx context.Context, round uint64, method string, args, rsp interface{}) error {
	raw, err := rc.cc.Query(ctx, &coreClient.QueryRequest{
		RuntimeID: rc.runtimeID,
		Round:     round,
		Method:    method,
		Args:      cbor.Marshal(args),
	})
	if err != nil {
		return err
	}
	if err = cbor.Unmarshal(raw.Data, rsp); err != nil {
		return fmt.Errorf("failed to unmarshal response: %w", err)
	}
	return nil
}

// New creates a new runtime client for the specified runtime.
func New(conn *grpc.ClientConn, runtimeID common.Namespace) RuntimeClient {
	return &runtimeClient{
		cs:        consensus.NewConsensusClient(conn),
		cc:        coreClient.NewRuntimeClient(conn),
		runtimeID: runtimeID,
	}
}
