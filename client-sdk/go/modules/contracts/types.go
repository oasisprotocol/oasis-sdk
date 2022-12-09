package contracts

import (
	"encoding/binary"
	"fmt"

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

// String returns a string representation of an ABI.
func (a ABI) String() string {
	switch a {
	case ABIOasisV1:
		return "Oasis v1"
	default:
		return "[unknown]"
	}
}

// Code is stored code information.
type Code struct {
	// ID is the unique code identifier.
	ID CodeID `json:"id"`
	// Hash is the code hash.
	Hash hash.Hash `json:"hash"`
	// ABI.
	ABI ABI `json:"abi"`
	// ABI sub-version.
	ABISubVersion uint32 `json:"abi_sv,omitempty"`
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
	// Code is the compressed compiled contract code.
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

// ChangeUpgradePolicy is the body of the contracts.ChangeUpgradePolicy call.
type ChangeUpgradePolicy struct {
	// ID is the unique instance identifier.
	ID InstanceID `json:"id"`
	// UpgradesPolicy is the updated upgrade policy.
	UpgradesPolicy Policy `json:"upgrades_policy"`
}

// CodeQuery is the body of the contracts.Code query.
type CodeQuery struct {
	// ID is the code identifier.
	ID CodeID `json:"id"`
}

// CodeStorageQuery is the body of the contracts.CodeStorage query.
type CodeStorageQuery struct {
	// ID is the code identifier.
	ID CodeID `json:"id"`
}

// CodeStorageQueryResult is the result of the contracts.CodeStorage query.
type CodeStorageQueryResult struct {
	// Code is the stored contract code.
	Code []byte `json:"code"`
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

// StoreKind defines the public or confidential store type for performing queries.
type StoreKind uint32

const (
	// StoreKindPublicName is a human-readable name for public store kind.
	StoreKindPublicName = "public"

	// StoreKindConfidentialName is a human-readable name for confidential store kind.
	StoreKindConfidentialName = "confidential"
)

// MarshalText returns human-readable name of StoreKind.
func (sk StoreKind) MarshalText() (data []byte, err error) {
	switch sk {
	case StoreKindPublic:
		return []byte(StoreKindPublicName), nil
	case StoreKindConfidential:
		return []byte(StoreKindConfidentialName), nil
	}

	return nil, fmt.Errorf("unsupported store kind '%d'", sk)
}

// UnmarshalText converts human-readable name of store kind to StoreKind.
func (sk *StoreKind) UnmarshalText(s []byte) error {
	switch string(s) {
	case StoreKindPublicName:
		*sk = StoreKindPublic
	case StoreKindConfidentialName:
		*sk = StoreKindConfidential
	default:
		return fmt.Errorf("unsupported store kind name '%v'", sk)
	}

	return nil
}

// These constants represent the kinds of store that the queries support.
const (
	StoreKindPublic       StoreKind = 0
	StoreKindConfidential StoreKind = 1
)

// InstanceRawStorageQuery is the body of the contracts.InstanceRawStorage query.
type InstanceRawStorageQuery struct {
	// ID is the instance identifier.
	ID InstanceID `json:"id"`

	// StoreKind is type of store to query.
	StoreKind StoreKind `json:"store_kind"`

	// Limit is the maximum number of items per page.
	Limit uint64 `json:"limit,omitempty"`

	// Offset is the number of skipped items.
	Offset uint64 `json:"offset,omitempty"`
}

// InstanceRawStorageQueryResult is the result of the contracts.InstanceRawStorage query.
type InstanceRawStorageQueryResult struct {
	// Items is a list of key-value pairs in contract's public store.
	Items []InstanceStorageKeyValue `json:"items"`
}

// InstanceStorageKeyValue is used as a tuple type for the contract storage.
type InstanceStorageKeyValue struct {
	_ struct{} `cbor:",toarray"`

	Key   []byte
	Value []byte
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

// GasCosts are the contracts module gas costs.
type GasCosts struct {
	TxUpload              uint64 `json:"tx_upload"`
	TxUploadPerByte       uint64 `json:"tx_upload_per_byte"`
	TxInstantiate         uint64 `json:"tx_instantiate"`
	TxCall                uint64 `json:"tx_call"`
	TxUpgrade             uint64 `json:"tx_upgrade"`
	TxChangeUpgradePolicy uint64 `json:"tx_change_upgrade_policy"`

	SubcallDispatch uint64 `json:"subcall_dispatch"`

	WASMPublicStorageGetBase          uint64 `json:"wasm_public_storage_get_base"`
	WASMPublicStorageInsertBase       uint64 `json:"wasm_public_storage_insert_base"`
	WASMPublicStorageRemoveBase       uint64 `json:"wasm_public_storage_remove_base"`
	WASMPublicStorageKeyByte          uint64 `json:"wasm_public_storage_key_byte"`
	WASMPublicStorageValueByte        uint64 `json:"wasm_public_storage_value_byte"`
	WASMConfidentialStorageGetBase    uint64 `json:"wasm_confidential_storage_get_base"`
	WASMConfidentialStorageInsertBase uint64 `json:"wasm_confidential_storage_insert_base"`
	WASMConfidentialStorageRemoveBase uint64 `json:"wasm_confidential_storage_remove_base"`
	WASMConfidentialStorageKeyByte    uint64 `json:"wasm_confidential_storage_key_byte"`
	WASMConfidentialStorageValueByte  uint64 `json:"wasm_confidential_storage_value_byte"`
	WASMEnvQueryBase                  uint64 `json:"wasm_env_query_base"`

	WASMCryptoECDSARecover             uint64 `json:"wasm_crypto_ecdsa_recover"`
	WASMCryptoSignatureVerifyEd25519   uint64 `json:"wasm_crypto_signature_verify_ed25519"`
	WASMCryptoSignatureVerifySecp256k1 uint64 `json:"wasm_crypto_signature_verify_secp256k1"`
	WASMCryptoSignatureVerifySr25519   uint64 `json:"wasm_crypto_signature_verify_sr25519"`
	WASMCryptoX25519DeriveSymmetric    uint64 `json:"wasm_crypto_x25519_derive_symmetric"`
	WASMCryptoDeoxysIIBase             uint64 `json:"wasm_crypto_deoxysii_base"`
	WASMCryptoDeoxysIIByte             uint64 `json:"wasm_crypto_deoxysii_byte"`
	WASMCryptoRandomBytesBase          uint64 `json:"wasm_crypto_random_bytes_base"`
	WASMCryptoRandomBytesByte          uint64 `json:"wasm_crypto_random_bytes_byte"`
}

// Parameters are the parameters for the contracts module.
type Parameters struct {
	MaxCodeSize    uint32 `json:"max_code_size"`
	MaxStackSize   uint32 `json:"max_stack_size"`
	MaxMemoryPages uint32 `json:"max_memory_pages"`

	MaxWASMFunctions uint32 `json:"max_wasm_functions"`
	MaxWASMLocals    uint32 `json:"max_wasm_locals"`

	MaxSubcallDepth uint16 `json:"max_subcall_depth"`
	MaxSubcallCount uint16 `json:"max_subcall_count"`

	MaxResultSizeBytes                       uint32 `json:"max_result_size_bytes"`
	MaxQuerySizeBytes                        uint32 `json:"max_query_size_bytes"`
	MaxStorageKeySizeBytes                   uint32 `json:"max_storage_key_size_bytes"`
	MaxStorageValueSizeBytes                 uint32 `json:"max_storage_value_size_bytes"`
	MaxCryptoSignatureVerifyMessageSizeBytes uint32 `json:"max_crypto_signature_verify_message_size_bytes"`

	GasCosts GasCosts `json:"gas_costs"`
}

// ModuleName is the contracts module name.
const ModuleName = "contracts"

// Event is an event emitted by a contract.
type Event struct {
	// ID is the instance identifier.
	ID InstanceID `json:"id"`
	// Data is the cbor serialized event data.
	Data []byte `json:"data,omitempty"`
}
