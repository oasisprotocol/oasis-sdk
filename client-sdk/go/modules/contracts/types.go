package contracts

import (
	"encoding/binary"

	"github.com/oasisprotocol/oasis-core/go/common/crypto/hash"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

// CodeID is the unique stored code identifier.
type CodeID uint64

// InstanceID is the unique deployed code instance identifier.
type InstanceID uint64

// Address returns address for the InstanceID.
func (i *InstanceID) Address() types.Address {
	id := make([]byte, 8)
	binary.BigEndian.PutUint64(id, uint64(*i))
	return types.NewAddressForModule("contracts", id)
}

// Policy is a generic policy that specifies who is allowed to perform an action.
type Policy struct {
	Nobody   *struct{}      `json:"nobody,omitempty"`
	Address  *types.Address `json:"address,omitempty"`
	Everyone *struct{}      `json:"everyone,omitempty"`
}

// ABI is the ABI that the given contract should conform to.
type ABI uint8

const (
	// ABIOasisV1 is the custom Oasis SDK-specific ABI (v1).
	ABIOasisV1 = ABI(1)
)

// Code is stored code information.
type Code struct {
	// ID is the unique code identifier.
	ID CodeID `json:"id"`
	// Hash is the code hash.
	Hash hash.Hash `json:"hash"`
	// ABI.
	ABI ABI `json:"abi"`
	// Uploader is the code uploader address.
	Uploader types.Address `json:"uploader"`
	// InstantiatePolicy is the policy on who is allowed to instantiate this code.
	InstantiatePolicy Policy `json:"instantiate_policy"`
}

// Instance is deployed code instance information.
type Instance struct {
	// ID is the unique instance identifier.
	ID InstanceID `json:"id"`
	// CodeID is the identifier of code used by the instance.
	CodeID CodeID `json:"code_id"`
	// Creator is the instance creator address.
	Creator types.Address `json:"creator"`
	// UpgradesPolicy is the policy on who is allowed to upgrade this instance.
	UpgradesPolicy Policy `json:"upgrades_policy"`
}

// Upload is the body of the contracts.Upload call.
type Upload struct {
	// ABI.
	ABI ABI `json:"abi"`
	// InstantiatePolicy is the policy on Who is allowed to instantiate this code.
	InstantiatePolicy Policy `json:"instantiate_policy"`
	// Code is the compiled contract code.
	Code []byte `json:"code"`
}

// UploadResult is the result of the contracts.Upload call.
type UploadResult struct {
	// ID is the assigned code identifier.
	ID CodeID `json:"id"`
}

// Instantiate is the body of the contracts.Instantiate call.
type Instantiate struct {
	// CodeID is the identifier of code used by the instance.
	CodeID CodeID `json:"code_id"`
	// UpgradesPolicy is the policy on who is allowed to upgrade this instance.
	UpgradesPolicy Policy `json:"upgrades_policy"`
	// Data are the arguments to contract's instantiation function.
	Data []byte `json:"data"`
	// Tokens that should be sent to the contract as part of the instantiate call.
	Tokens []types.BaseUnits `json:"tokens"`
}

// InstantiateResult is the result of the contracts.Instantiate call.
type InstantiateResult struct {
	// ID is the assigned instance identifier.
	ID InstanceID `json:"id"`
}

// Call is the body of the contracts.Call call.
type Call struct {
	// ID is the instance identifier.
	ID InstanceID `json:"id"`
	// Data are the arguments to contract's instantiation function.
	Data []byte `json:"data"`
	// Tokens that should be sent to the contract as part of the call.
	Tokens []types.BaseUnits `json:"tokens"`
}

// CallResult is the result of the contracts.Call call.
type CallResult []byte

// Upgrade is the body of the contracts.Upgrade call.
type Upgrade struct {
	// ID is the instance identifier.
	ID InstanceID `json:"id"`
	// CodeID is the identifier of updated code to be used by the instance.
	CodeID CodeID `json:"code_id"`
	// Data are the arguments to contract's instantiation function.
	Data []byte `json:"data"`
	// Tokens that should be sent to the contract as part of the upgrade call.
	Tokens []types.BaseUnits `json:"tokens"`
}

// CodeQuery is the body of the contracts.Code query.
type CodeQuery struct {
	// ID is the code identifier.
	ID CodeID `json:"id"`
}

// InstanceQuery is the body of the contracts.Instance query.
type InstanceQuery struct {
	// ID is the instance identifier.
	ID InstanceID `json:"id"`
}

// InstanceStorageQuery is the body of the contracts.InstanceStorage query.
type InstanceStorageQuery struct {
	// ID is the instance identifier.
	ID InstanceID `json:"id"`
	// Key is the storage key.
	Key []byte `json:"key"`
}

// InstanceStorageQueryResult is the result of the contracts.InstanceStorage query.
type InstanceStorageQueryResult struct {
	// Value is the storage value or nil if key doesn't exist.
	Value []byte `json:"value"`
}

// PublicKeyKind is the public key kind.
type PublicKeyKind uint8

const (
	// PublicKeyTransaction is the transaction public key kind.
	PublicKeyTransaction = PublicKeyKind(1)
)

// PublicKeyQuery is the body of the contracts.PublicKey query.
type PublicKeyQuery struct {
	// ID is the instance identifier.
	ID InstanceID `json:"id"`
	// Kind is the public key kind.
	Kind PublicKeyKind `json:"kind"`
}

// PublicKeyQueryResult is the result of the contracts.PublicKey query.
type PublicKeyQueryResult struct {
	// Key is the public key.
	Key []byte `json:"key"`
	// Checksum of the key manager state.
	Checksum []byte `json:"checksum"`
	// Signature is the Sign(sk, (key || checksum)) from the key manager.
	Signature []byte `json:"signature"`
}

// CustomQuery is the body of the contracts.Custom query.
type CustomQuery struct {
	// ID is the instance identifier.
	ID InstanceID `json:"id"`
	// Data are the query method arguments.
	Data []byte `json:"data"`
}

// CustomQueryResult is the result of the contracts.Custom query.
type CustomQueryResult []byte
