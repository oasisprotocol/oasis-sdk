package wallet

import (
	"fmt"
	"sync"

	"github.com/AlecAivazis/survey/v2"
	ethCommon "github.com/ethereum/go-ethereum/common"
	flag "github.com/spf13/pflag"

	coreSignature "github.com/oasisprotocol/oasis-core/go/common/crypto/signature"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

var registeredFactories sync.Map

const (
	// AlgorithmEd25519Adr8 is the Ed25519 algorithm using the ADR-0008 derivation.
	AlgorithmEd25519Adr8 = "ed25519-adr8"
	// AlgorithmEd25519Raw is the Ed25519 algorithm using raw private keys.
	AlgorithmEd25519Raw = "ed25519-raw"
	// AlgorithmSecp256k1Bip44 is the Secp256k1 algorithm using BIP-44 derivation.
	AlgorithmSecp256k1Bip44 = "secp256k1-bip44"
	// AlgorithmSecp256k1Raw is the Secp256k1 algorithm using raw private keys.
	AlgorithmSecp256k1Raw = "secp256k1-raw"
)

// Factory is a factory that supports accounts of a specific kind.
type Factory interface {
	// Kind returns the kind of accounts this factory will produce.
	Kind() string

	// PrettyKind returns human-friendly kind of accounts this factory will produce.
	PrettyKind(cfg map[string]interface{}) string

	// Flags returns the CLI flags that can be used for configuring this account factory.
	Flags() *flag.FlagSet

	// GetConfigFromFlags generates account configuration from flags.
	GetConfigFromFlags() (map[string]interface{}, error)

	// GetConfigFromSurvey generates account configuration from survey answers.
	GetConfigFromSurvey(kind *ImportKind) (map[string]interface{}, error)

	// DataPrompt returns a survey prompt for entering data when importing the account.
	DataPrompt(kind ImportKind, cfg map[string]interface{}) survey.Prompt

	// DataValidator returns a survey data input validator used when importing the account.
	DataValidator(kind ImportKind, cfg map[string]interface{}) survey.Validator

	// RequiresPassphrase returns true if the account requires a passphrase.
	RequiresPassphrase() bool

	// SupportedImportKinds returns the import kinds supported by this account.
	SupportedImportKinds() []ImportKind

	// HasConsensusSigner returns true, iff there is a consensus layer signer associated with this account.
	HasConsensusSigner(cfg map[string]interface{}) bool

	// Create creates a new account.
	Create(name string, passphrase string, cfg map[string]interface{}) (Account, error)

	// Load loads an existing account.
	Load(name string, passphrase string, cfg map[string]interface{}) (Account, error)

	// Remove removes an existing account.
	Remove(name string, cfg map[string]interface{}) error

	// Rename renames an existing account.
	Rename(old, new string, cfg map[string]interface{}) error

	// Import creates a new account from imported key material.
	Import(name string, passphrase string, cfg map[string]interface{}, src *ImportSource) (Account, error)
}

// ImportKind is an account import kind.
type ImportKind string

// Supported import kinds.
const (
	ImportKindMnemonic   ImportKind = "mnemonic"
	ImportKindPrivateKey ImportKind = "private key"
)

// UnmarshalText decodes a text marshalled import kind.
func (k *ImportKind) UnmarshalText(text []byte) error {
	switch string(text) {
	case string(ImportKindMnemonic):
		*k = ImportKindMnemonic
	case string(ImportKindPrivateKey):
		*k = ImportKindPrivateKey
	default:
		return fmt.Errorf("unknown import kind: %s", string(text))
	}
	return nil
}

// ImportSource is a source of imported account key material.
type ImportSource struct {
	Kind ImportKind
	Data string
}

// Account is an interface of a single account in the wallet.
type Account interface {
	// ConsensusSigner returns the consensus layer signer associated with the account.
	//
	// It may return nil in case this account cannot be used with the consensus layer.
	ConsensusSigner() coreSignature.Signer

	// Signer returns the signer associated with the account.
	Signer() signature.Signer

	// Address returns the address associated with the account.
	Address() types.Address

	// EthAddress returns the Ethereum address associated with the account, if any.
	EthAddress() *ethCommon.Address

	// SignatureAddressSpec returns the signature address specification associated with the account.
	SignatureAddressSpec() types.SignatureAddressSpec

	// UnsafeExport exports the account's secret state.
	UnsafeExport() string
}

// Register registers a new account type.
func Register(af Factory) {
	if _, loaded := registeredFactories.LoadOrStore(af.Kind(), af); loaded {
		panic(fmt.Sprintf("wallet: kind '%s' is already registered", af.Kind()))
	}
}

// Load loads a previously registered account factory.
func Load(kind string) (Factory, error) {
	af, loaded := registeredFactories.Load(kind)
	if !loaded {
		return nil, fmt.Errorf("wallet: kind '%s' not available", kind)
	}
	return af.(Factory), nil
}

// AvailableKinds returns all of the available account factories.
func AvailableKinds() []Factory {
	var kinds []Factory
	registeredFactories.Range(func(key, value interface{}) bool {
		kinds = append(kinds, value.(Factory))
		return true
	})
	return kinds
}

// ImportKinds returns all of the available account import kinds.
func ImportKinds() []string {
	return []string{
		string(ImportKindMnemonic),
		string(ImportKindPrivateKey),
	}
}
