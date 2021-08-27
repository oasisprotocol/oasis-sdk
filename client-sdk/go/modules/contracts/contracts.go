package contracts

import (
	"context"
	"fmt"

	"github.com/oasisprotocol/oasis-core/go/common/cbor"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/client"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

const (
	// Callable methods.
	methodUpload      = "contracts.Upload"
	methodInstantiate = "contracts.Instantiate"
	methodCall        = "contracts.Call"
	methodUpgrade     = "contracts.Upgrade"

	// Queries.
	methodCode            = "contracts.Code"
	methodInstance        = "contracts.Instance"
	methodInstanceStorage = "contracts.InstanceStorage"
	methodPublicKey       = "contracts.PublicKey"
	methodCustom          = "contracts.Custom"
)

// V1 is the v1 contracts module interface.
type V1 interface {
	// Upload generates a contracts.Upload transaction.
	Upload(abi ABI, instantiatePolicy Policy, code []byte) *client.TransactionBuilder

	// InstantiateRaw generates a contracts.Instantiate transaction.
	//
	// This method allows specifying an arbitrary data payload. If the contract is using the Oasis
	// ABI you can use the regular Call method as convenience since it will perform the CBOR
	// serialization automatically.
	InstantiateRaw(codeID CodeID, upgradesPolicy Policy, data []byte, tokens []types.BaseUnits) *client.TransactionBuilder

	// Instantiate generates a contracts.Instantiate transaction.
	//
	// This method will encode the specified data using CBOR as defined by the Oasis ABI.
	Instantiate(codeID CodeID, upgradesPolicy Policy, data interface{}, tokens []types.BaseUnits) *client.TransactionBuilder

	// CallRaw generates a contracts.Call transaction.
	//
	// This method allows specifying an arbitrary data payload. If the contract is using the Oasis
	// ABI you can use the regular Call method as convenience since it will perform the CBOR
	// serialization automatically.
	CallRaw(id InstanceID, data []byte, tokens []types.BaseUnits) *client.TransactionBuilder

	// Call generates a contracts.Call transaction.
	//
	// This method will encode the specified data using CBOR as defined by the Oasis ABI.
	Call(id InstanceID, data interface{}, tokens []types.BaseUnits) *client.TransactionBuilder

	// UpgradeRaw generates a contracts.Upgrade transaction.
	//
	// This method allows specifying an arbitrary data payload. If the contract is using the Oasis
	// ABI you can use the regular Upgrade method as convenience since it will perform the CBOR
	// serialization automatically.
	UpgradeRaw(id InstanceID, codeID CodeID, data []byte, tokens []types.BaseUnits) *client.TransactionBuilder

	// Upgrade generates a contracts.Upgrade transaction.
	//
	// This method will encode the specified data using CBOR as defined by the Oasis ABI.
	Upgrade(id InstanceID, codeID CodeID, data interface{}, tokens []types.BaseUnits) *client.TransactionBuilder

	// Code queries the given code information.
	Code(ctx context.Context, round uint64, id CodeID) (*Code, error)

	// Instance queries the given instance information.
	Instance(ctx context.Context, round uint64, id InstanceID) (*Instance, error)

	// InstanceStorage queries the given instance's storage.
	InstanceStorage(ctx context.Context, round uint64, id InstanceID, key []byte) (*InstanceStorageQueryResult, error)

	// PublicKey queries the given instance's public key.
	PublicKey(ctx context.Context, round uint64, id InstanceID, kind PublicKeyKind) (*PublicKeyQueryResult, error)

	// CustomRaw queries the given contract for a custom query.
	//
	// This method allows specifying an arbitrary data payload. If the contract is using the Oasis
	// ABI you can use the regular Custom method as convenience since it will perform the CBOR
	// serialization automatically.
	CustomRaw(ctx context.Context, round uint64, id InstanceID, data []byte) ([]byte, error)

	// Custom queries the given contract for a custom query.
	//
	// This method will encode the specified data using CBOR as defined by the Oasis ABI.
	Custom(ctx context.Context, round uint64, id InstanceID, data, rsp interface{}) error
}

type v1 struct {
	rc client.RuntimeClient
}

// Implements V1.
func (a *v1) Upload(abi ABI, instantiatePolicy Policy, code []byte) *client.TransactionBuilder {
	return client.NewTransactionBuilder(a.rc, methodUpload, &Upload{
		ABI:               abi,
		InstantiatePolicy: instantiatePolicy,
		Code:              code,
	})
}

// Implements V1.
func (a *v1) InstantiateRaw(codeID CodeID, upgradesPolicy Policy, data []byte, tokens []types.BaseUnits) *client.TransactionBuilder {
	return client.NewTransactionBuilder(a.rc, methodInstantiate, &Instantiate{
		CodeID:         codeID,
		UpgradesPolicy: upgradesPolicy,
		Data:           data,
		Tokens:         tokens,
	})
}

// Implements V1.
func (a *v1) Instantiate(codeID CodeID, upgradesPolicy Policy, data interface{}, tokens []types.BaseUnits) *client.TransactionBuilder {
	return a.InstantiateRaw(codeID, upgradesPolicy, cbor.Marshal(data), tokens)
}

// Implements V1.
func (a *v1) CallRaw(id InstanceID, data []byte, tokens []types.BaseUnits) *client.TransactionBuilder {
	return client.NewTransactionBuilder(a.rc, methodCall, &Call{
		ID:     id,
		Data:   data,
		Tokens: tokens,
	})
}

// Implements V1.
func (a *v1) Call(id InstanceID, data interface{}, tokens []types.BaseUnits) *client.TransactionBuilder {
	return a.CallRaw(id, cbor.Marshal(data), tokens)
}

// Implements V1.
func (a *v1) UpgradeRaw(id InstanceID, codeID CodeID, data []byte, tokens []types.BaseUnits) *client.TransactionBuilder {
	return client.NewTransactionBuilder(a.rc, methodUpgrade, &Upgrade{
		ID:     id,
		CodeID: codeID,
		Data:   data,
		Tokens: tokens,
	})
}

// Implements V1.
func (a *v1) Upgrade(id InstanceID, codeID CodeID, data interface{}, tokens []types.BaseUnits) *client.TransactionBuilder {
	return a.UpgradeRaw(id, codeID, cbor.Marshal(data), tokens)
}

// Implements V1.
func (a *v1) Code(ctx context.Context, round uint64, id CodeID) (*Code, error) {
	var code Code
	err := a.rc.Query(ctx, round, methodCode, &CodeQuery{ID: id}, &code)
	if err != nil {
		return nil, err
	}
	return &code, nil
}

// Implements V1.
func (a *v1) Instance(ctx context.Context, round uint64, id InstanceID) (*Instance, error) {
	var instance Instance
	err := a.rc.Query(ctx, round, methodInstance, &InstanceQuery{ID: id}, &instance)
	if err != nil {
		return nil, err
	}
	return &instance, nil
}

// Implements V1.
func (a *v1) InstanceStorage(ctx context.Context, round uint64, id InstanceID, key []byte) (*InstanceStorageQueryResult, error) {
	var rsp InstanceStorageQueryResult
	err := a.rc.Query(ctx, round, methodInstanceStorage, &InstanceStorageQuery{ID: id, Key: key}, &rsp)
	if err != nil {
		return nil, err
	}
	return &rsp, nil
}

// Implements V1.
func (a *v1) PublicKey(ctx context.Context, round uint64, id InstanceID, kind PublicKeyKind) (*PublicKeyQueryResult, error) {
	var pk PublicKeyQueryResult
	err := a.rc.Query(ctx, round, methodPublicKey, &PublicKeyQuery{ID: id, Kind: kind}, &pk)
	if err != nil {
		return nil, err
	}
	return &pk, nil
}

// Implements V1.
func (a *v1) CustomRaw(ctx context.Context, round uint64, id InstanceID, data []byte) ([]byte, error) {
	var rsp CustomQueryResult
	err := a.rc.Query(ctx, round, methodCustom, &CustomQuery{ID: id, Data: data}, &rsp)
	if err != nil {
		return nil, err
	}
	return []byte(rsp), nil
}

// Implements V1.
func (a *v1) Custom(ctx context.Context, round uint64, id InstanceID, data, rsp interface{}) error {
	raw, err := a.CustomRaw(ctx, round, id, cbor.Marshal(data))
	if err != nil {
		return err
	}
	if err = cbor.Unmarshal(raw, rsp); err != nil {
		return fmt.Errorf("failed to unmarshal response from contract: %w", err)
	}
	return nil
}

// NewV1 generates a V1 client helper for the contracts module.
func NewV1(rc client.RuntimeClient) V1 {
	return &v1{rc: rc}
}
