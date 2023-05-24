package contracts

import (
	"bytes"
	"context"
	"fmt"
	"strings"
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
	methodParameters      = "contracts.Parameters"
)

// V1 is the v1 contracts module interface.
type V1 interface {
	client.EventDecoder

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

	// Parameters queries the EVM module parameters.
	Parameters(ctx context.Context, round uint64) (*Parameters, error)

	// GetEvents returns events emitted by the contract at the provided round.
	GetEvents(ctx context.Context, instanceID InstanceID, round uint64) ([]*Event, error)
}

type v1 struct {
	rc client.RuntimeClient
}

// Implements V1.
func (a *v1) Upload(abi ABI, instantiatePolicy Policy, code []byte) *client.TransactionBuilder {
	return client.NewTransactionBuilder(a.rc, methodUpload, &Upload{
		ABI:               abi,
		InstantiatePolicy: instantiatePolicy,
		Code:              CompressCode(code),
	})
}
