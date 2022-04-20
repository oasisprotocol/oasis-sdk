package wallet

import (
	"fmt"
	"sync"

	"github.com/AlecAivazis/survey/v2"
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

// Factory is a factory that supports wallets of a specific kind.
type Factory interface {
	// Kind returns the kind of wallets this factory will produce.
	Kind() string

	// PrettyKind returns human-friendly kind of wallets this factory will produce.
	PrettyKind(cfg map[string]interface{}) string

	// Flags returns the CLI flags that can be used for configuring this wallet factory.
	Flags() *flag.FlagSet

	// GetConfigFromFlags generates wallet configuration from flags.
	GetConfigFromFlags() (map[string]interface{}, error)

	// GetConfigFromSurvey generates wallet configuration from survey answers.
	GetConfigFromSurvey(kind *ImportKind) (map[string]interface{}, error)

	// DataPrompt returns a survey prompt for entering data when importing the wallet.
	DataPrompt(kind ImportKind, cfg map[string]interface{}) survey.Prompt

	// DataValidator returns a survey data input validator used when importing the wallet.
	DataValidator(kind ImportKind, cfg map[string]interface{}) survey.Validator

	// RequiresPassphrase returns true if the wallet requires a passphrase.
	RequiresPassphrase() bool

	// SupportedImportKinds returns the import kinds supported by this wallet.
	SupportedImportKinds() []ImportKind

	// HasConsensusSigner returns true, iff there is a consensus layer signer associated with this wallet.
	HasConsensusSigner(cfg map[string]interface{}) bool

	// Create creates a new wallet.
	Create(name string, passphrase string, cfg map[string]interface{}) (Wallet, error)

	// Load loads an existing wallet.
	Load(name string, passphrase string, cfg map[string]interface{}) (Wallet, error)

	// Remove removes an existing wallet.
	Remove(name string, cfg map[string]interface{}) error

	// Rename renames an existing wallet.
	Rename(old, new string, cfg map[string]interface{}) error

	// Import creates a new wallet from imported key material.
	Import(name string, passphrase string, cfg map[string]interface{}, src *ImportSource) (Wallet, error)
}

// ImportKind is a wallet import kind.
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

// ImportSource is a source of imported wallet key material.
type ImportSource struct {
	Kind ImportKind
	Data string
}

// Wallet is the wallet interface.
type Wallet interface {
	// ConsensusSigner returns the consensus layer signer associated with the wallet.
	//
	// It may return nil in case this wallet cannot be used with the consensus layer.
	ConsensusSigner() coreSignature.Signer

	// Signer returns the signer associated with the wallet.
	Signer() signature.Signer

	// Address returns the address associated with the wallet.
	Address() types.Address

	// SignatureAddressSpec returns the signature address specification associated with the wallet.
	SignatureAddressSpec() types.SignatureAddressSpec

	// UnsafeExport exports the wallet's secret state.
	UnsafeExport() string
}

// Register registers a new wallet type.
func Register(wf Factory) {
	if _, loaded := registeredFactories.LoadOrStore(wf.Kind(), wf); loaded {
		panic(fmt.Sprintf("wallet: kind '%s' is already registered", wf.Kind()))
	}
}

// Load loads a previously registered wallet factory.
func Load(kind string) (Factory, error) {
	wf, loaded := registeredFactories.Load(kind)
	if !loaded {
		return nil, fmt.Errorf("wallet: kind '%s' not available", kind)
	}
	return wf.(Factory), nil
}

// AvailableKinds returns all of the available wallet factories.
func AvailableKinds() []Factory {
	var kinds []Factory
	registeredFactories.Range(func(key, value interface{}) bool {
		kinds = append(kinds, value.(Factory))
		return true
	})
	return kinds
}

// ImportKinds returns all of the available wallet import kinds.
func ImportKinds() []string {
	return []string{
		string(ImportKindMnemonic),
		string(ImportKindPrivateKey),
	}
}
