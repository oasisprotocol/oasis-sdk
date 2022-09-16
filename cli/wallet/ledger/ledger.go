package ledger

import (
	"fmt"

	"github.com/AlecAivazis/survey/v2"
	ethCommon "github.com/ethereum/go-ethereum/common"
	"github.com/mitchellh/mapstructure"
	flag "github.com/spf13/pflag"

	coreSignature "github.com/oasisprotocol/oasis-core/go/common/crypto/signature"

	"github.com/oasisprotocol/oasis-sdk/cli/wallet"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature/ed25519"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

const (
	// Kind is the account kind for the ledger-backed accounts.
	Kind = "ledger"

	derivationAdr8   = "adr8"
	derivationLegacy = "legacy"

	cfgDerivation = "ledger.derivation"
	cfgNumber     = "ledger.number"
)

type accountConfig struct {
	Derivation string `mapstructure:"derivation,omitempty"`
	Number     uint32 `mapstructure:"number,omitempty"`
}

type ledgerAccountFactory struct {
	flags *flag.FlagSet
}

func (af *ledgerAccountFactory) Kind() string {
	return Kind
}

func (af *ledgerAccountFactory) PrettyKind(rawCfg map[string]interface{}) string {
	cfg, err := af.unmarshalConfig(rawCfg)
	if err != nil {
		return ""
	}

	// Show adr8, if derivation not set.
	derivation := cfg.Derivation
	if derivation == "" {
		derivation = derivationAdr8
	}
	return fmt.Sprintf("%s (%s:%d)", af.Kind(), derivation, cfg.Number)
}

func (af *ledgerAccountFactory) Flags() *flag.FlagSet {
	return af.flags
}

func (af *ledgerAccountFactory) GetConfigFromFlags() (map[string]interface{}, error) {
	cfg := make(map[string]interface{})
	cfg["derivation"], _ = af.flags.GetString(cfgDerivation)
	cfg["number"], _ = af.flags.GetUint32(cfgNumber)
	return cfg, nil
}

func (af *ledgerAccountFactory) GetConfigFromSurvey(kind *wallet.ImportKind) (map[string]interface{}, error) {
	return nil, fmt.Errorf("ledger: import not supported")
}

func (af *ledgerAccountFactory) DataPrompt(kind wallet.ImportKind, rawCfg map[string]interface{}) survey.Prompt {
	return nil
}

func (af *ledgerAccountFactory) DataValidator(kind wallet.ImportKind, rawCfg map[string]interface{}) survey.Validator {
	return nil
}

func (af *ledgerAccountFactory) RequiresPassphrase() bool {
	return false
}

func (af *ledgerAccountFactory) SupportedImportKinds() []wallet.ImportKind {
	return []wallet.ImportKind{}
}

func (af *ledgerAccountFactory) HasConsensusSigner(rawCfg map[string]interface{}) bool {
	return true
}

func (af *ledgerAccountFactory) unmarshalConfig(raw map[string]interface{}) (*accountConfig, error) {
	if raw == nil {
		return nil, fmt.Errorf("missing configuration")
	}

	var cfg accountConfig
	if err := mapstructure.Decode(raw, &cfg); err != nil {
		return nil, err
	}
	return &cfg, nil
}

func (af *ledgerAccountFactory) Create(name string, passphrase string, rawCfg map[string]interface{}) (wallet.Account, error) {
	cfg, err := af.unmarshalConfig(rawCfg)
	if err != nil {
		return nil, err
	}

	return newAccount(cfg)
}

func (af *ledgerAccountFactory) Load(name string, passphrase string, rawCfg map[string]interface{}) (wallet.Account, error) {
	cfg, err := af.unmarshalConfig(rawCfg)
	if err != nil {
		return nil, err
	}

	return newAccount(cfg)
}

func (af *ledgerAccountFactory) Remove(name string, rawCfg map[string]interface{}) error {
	return nil
}

func (af *ledgerAccountFactory) Rename(old, new string, rawCfg map[string]interface{}) error {
	return nil
}

func (af *ledgerAccountFactory) Import(name string, passphrase string, rawCfg map[string]interface{}, src *wallet.ImportSource) (wallet.Account, error) {
	return nil, fmt.Errorf("ledger: import not supported")
}

type ledgerAccount struct {
	cfg        *accountConfig
	signer     *ledgerSigner
	coreSigner *ledgerCoreSigner
}

func newAccount(cfg *accountConfig) (wallet.Account, error) {
	// Connect to device.
	dev, err := connectToDevice()
	if err != nil {
		return nil, err
	}

	var path []uint32
	switch cfg.Derivation {
	case derivationAdr8, "":
		path = getAdr0008Path(cfg.Number)
	case derivationLegacy:
		path = getLegacyPath(cfg.Number)
	default:
		return nil, fmt.Errorf("ledger: unsupported derivation scheme '%s'", cfg.Derivation)
	}

	// Retrieve public key.
	rawPk, err := dev.GetPublicKeyEd25519(path, false)
	if err != nil {
		_ = dev.Close()
		return nil, err
	}

	// Create consensus layer signer.
	coreSigner := &ledgerCoreSigner{
		path: path,
		dev:  dev,
	}
	if err = coreSigner.pk.UnmarshalBinary(rawPk); err != nil {
		_ = dev.Close()
		return nil, fmt.Errorf("ledger: got malformed public key: %w", err)
	}

	// Create paratime layer signer.
	// NOTE: Ledger currently doesn't support signing paratime transactions.
	signer := &ledgerSigner{
		dev: dev,
	}
	if err = signer.pk.UnmarshalBinary(rawPk); err != nil {
		_ = dev.Close()
		return nil, fmt.Errorf("ledger: got malformed public key: %w", err)
	}

	return &ledgerAccount{
		cfg:        cfg,
		signer:     signer,
		coreSigner: coreSigner,
	}, nil
}

func (a *ledgerAccount) ConsensusSigner() coreSignature.Signer {
	return a.coreSigner
}

func (a *ledgerAccount) Signer() signature.Signer {
	return a.signer
}

func (a *ledgerAccount) Address() types.Address {
	return types.NewAddress(a.SignatureAddressSpec())
}

func (a *ledgerAccount) EthAddress() *ethCommon.Address {
	// secp256k1 accounts are not supported by Ledger yet.
	return nil
}

func (a *ledgerAccount) SignatureAddressSpec() types.SignatureAddressSpec {
	return types.NewSignatureAddressSpecEd25519(a.signer.Public().(ed25519.PublicKey))
}

func (a *ledgerAccount) UnsafeExport() string {
	return ""
}

func init() {
	flags := flag.NewFlagSet("", flag.ContinueOnError)
	flags.String(cfgDerivation, derivationLegacy, "Derivation scheme to use [adr8, legacy]")
	flags.Uint32(cfgNumber, 0, "Key number to use in the derivation scheme")

	wallet.Register(&ledgerAccountFactory{
		flags: flags,
	})
}
