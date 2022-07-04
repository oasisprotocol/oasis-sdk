package contracts

import (
	"bytes"
	"context"
	"fmt"
	"strings"

	"github.com/golang/snappy"

	"github.com/oasisprotocol/oasis-core/go/common/cbor"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/client"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

const (
	// Callable methods.
	methodUpload              = "contracts.Upload"
	methodInstantiate         = "contracts.Instantiate"
	methodCall                = "contracts.Call"
	methodUpgrade             = "contracts.Upgrade"
	methodChangeUpgradePolicy = "contracts.ChangeUpgradePolicy"

	// Queries.
	methodCode               = "contracts.Code"
	methodCodeStorage        = "contracts.CodeStorage"
	methodInstance           = "contracts.Instance"
	methodInstanceStorage    = "contracts.InstanceStorage"
	methodInstanceRawStorage = "contracts.InstanceRawStorage"
	methodPublicKey          = "contracts.PublicKey"
	methodCustom             = "contracts.Custom"
	methodParameters         = "contracts.Parameters"
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

	// ChangeUpgradePolicy generates a contracts.ChangeUpgradePolicy transaction.
	ChangeUpgradePolicy(id InstanceID, upgradesPolicy Policy) *client.TransactionBuilder

	// Code queries the given code information.
	Code(ctx context.Context, round uint64, id CodeID) (*Code, error)

	// CodeStorage queries the given code's storage.
	CodeStorage(ctx context.Context, round uint64, id CodeID) (*CodeStorageQueryResult, error)

	// Instance queries the given instance information.
	Instance(ctx context.Context, round uint64, id InstanceID) (*Instance, error)

	// InstanceStorage queries the given instance's public storage.
	InstanceStorage(ctx context.Context, round uint64, id InstanceID, key []byte) (*InstanceStorageQueryResult, error)

	// InstanceRawStorage returns the key-value pairs of contract instance storage.
	InstanceRawStorage(ctx context.Context, round uint64, id InstanceID, kind StoreKind, limit, offset uint64) (*InstanceRawStorageQueryResult, error)

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
func (a *v1) ChangeUpgradePolicy(id InstanceID, upgradesPolicy Policy) *client.TransactionBuilder {
	return client.NewTransactionBuilder(a.rc, methodChangeUpgradePolicy, &ChangeUpgradePolicy{
		ID:             id,
		UpgradesPolicy: upgradesPolicy,
	})
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
func (a *v1) CodeStorage(ctx context.Context, round uint64, id CodeID) (*CodeStorageQueryResult, error) {
	var rsp CodeStorageQueryResult
	err := a.rc.Query(ctx, round, methodCodeStorage, &CodeStorageQuery{ID: id}, &rsp)
	if err != nil {
		return nil, err
	}
	return &rsp, nil
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
func (a *v1) InstanceRawStorage(ctx context.Context, round uint64, id InstanceID, storeKind StoreKind, limit uint64, offset uint64) (*InstanceRawStorageQueryResult, error) {
	var rsp InstanceRawStorageQueryResult
	err := a.rc.Query(ctx, round, methodInstanceRawStorage, &InstanceRawStorageQuery{ID: id, StoreKind: storeKind, Limit: limit, Offset: offset}, &rsp)
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

// Implements V1.
func (a *v1) Parameters(ctx context.Context, round uint64) (*Parameters, error) {
	var params Parameters
	err := a.rc.Query(ctx, round, methodParameters, nil, &params)
	if err != nil {
		return nil, err
	}
	return &params, nil
}

// Implements V1.
func (a *v1) GetEvents(ctx context.Context, instanceID InstanceID, round uint64) ([]*Event, error) {
	rawEvs, err := a.rc.GetEventsRaw(ctx, round)
	if err != nil {
		return nil, err
	}

	evs := make([]*Event, 0)
	for _, rawEv := range rawEvs {
		ev, err := a.DecodeEvent(rawEv)
		if err != nil {
			return nil, err
		}
		for _, e := range ev {
			if e.(*Event).ID != instanceID {
				continue
			}
			evs = append(evs, e.(*Event))
		}
	}

	return evs, nil
}

// Implements client.EventDecoder.
func (a *v1) DecodeEvent(event *types.Event) ([]client.DecodedEvent, error) {
	return DecodeEvent(event)
}

// DecodeEvent decodes a contract event.
func DecodeEvent(event *types.Event) ([]client.DecodedEvent, error) {
	// "contracts" or "contracts.<...>".
	if event.Module != ModuleName && !strings.HasPrefix(event.Module, ModuleName+".") {
		return nil, nil
	}
	var evs []*Event
	if err := cbor.Unmarshal(event.Value, &evs); err != nil {
		return nil, fmt.Errorf("decode contract event value: %w", err)
	}
	events := make([]client.DecodedEvent, len(evs))
	for i, ev := range evs {
		events[i] = ev
	}
	return events, nil
}

// NewV1 generates a V1 client helper for the contracts module.
func NewV1(rc client.RuntimeClient) V1 {
	return &v1{rc: rc}
}

// NewUploadTx generates a new contracts.Upload transaction.
func NewUploadTx(fee *types.Fee, body *Upload) *types.Transaction {
	return types.NewTransaction(fee, methodUpload, body)
}

// NewInstantiateTx generates a new contracts.Instantiate transaction.
func NewInstantiateTx(fee *types.Fee, body *Instantiate) *types.Transaction {
	return types.NewTransaction(fee, methodInstantiate, body)
}

// NewCallTx generates a new contracts.Call transaction.
func NewCallTx(fee *types.Fee, body *Call) *types.Transaction {
	return types.NewTransaction(fee, methodCall, body)
}

// NewUpgradeTx generates a new contracts.Upgrade transaction.
func NewUpgradeTx(fee *types.Fee, body *Upgrade) *types.Transaction {
	return types.NewTransaction(fee, methodUpgrade, body)
}

// NewChangeUpgradePolicyTx generates a new contracts.ChangeUpgradePolicy transaction.
func NewChangeUpgradePolicyTx(fee *types.Fee, body *ChangeUpgradePolicy) *types.Transaction {
	return types.NewTransaction(fee, methodChangeUpgradePolicy, body)
}

// CompressCode performs code compression using Snappy.
func CompressCode(code []byte) []byte {
	var compressedCode bytes.Buffer
	encoder := snappy.NewBufferedWriter(&compressedCode)
	_, err := encoder.Write(code)
	if err != nil {
		panic(err)
	}
	encoder.Close()
	return compressedCode.Bytes()
}
