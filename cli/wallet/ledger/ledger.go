package ledger

import (
	"fmt"

	"github.com/mitchellh/mapstructure"
	flag "github.com/spf13/pflag"

	coreSignature "github.com/oasisprotocol/oasis-core/go/common/crypto/signature"

	"github.com/oasisprotocol/oasis-sdk/cli/wallet"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature/ed25519"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

const (
	// Kind is the wallet kind for the ledger-backed wallet.
	Kind = "ledger"

	derivationAdr8   = "adr8"
	derivationLegacy = "legacy"

	cfgDerivation = "ledger.derivation"
	cfgNumber     = "ledger.number"
)

type walletConfig struct {
	Derivation string `mapstructure:"derivation,omitempty"`
	Number     uint32 `mapstructure:"number,omitempty"`
}

type ledgerWalletFactory struct {
	flags *flag.FlagSet
}

func (wf *ledgerWalletFactory) Kind() string {
	return Kind
}

func (wf *ledgerWalletFactory) Flags() *flag.FlagSet {
	return wf.flags
}

func (wf *ledgerWalletFactory) GetConfigFromFlags() (map[string]interface{}, error) {
	cfg := make(map[string]interface{})
	cfg["derivation"], _ = wf.flags.GetString(cfgDerivation)
	cfg["number"], _ = wf.flags.GetUint32(cfgNumber)
	return cfg, nil
}

func (wf *ledgerWalletFactory) GetConfigFromSurvey(kind *wallet.ImportKind) (map[string]interface{}, error) {
	return nil, fmt.Errorf("ledger: import not supported")
}

func (wf *ledgerWalletFactory) RequiresPassphrase() bool {
	return false
}

func (wf *ledgerWalletFactory) SupportedImportKinds() []wallet.ImportKind {
	return []wallet.ImportKind{}
}

func (wf *ledgerWalletFactory) unmarshalConfig(raw map[string]interface{}) (*walletConfig, error) {
	if raw == nil {
		return nil, fmt.Errorf("missing configuration")
	}

	var cfg walletConfig
	if err := mapstructure.Decode(raw, &cfg); err != nil {
		return nil, err
	}
	return &cfg, nil
}

func (wf *ledgerWalletFactory) Create(name string, passphrase string, rawCfg map[string]interface{}) (wallet.Wallet, error) {
	cfg, err := wf.unmarshalConfig(rawCfg)
	if err != nil {
		return nil, err
	}

	return newWallet(cfg)
}

func (wf *ledgerWalletFactory) Load(name string, passphrase string, rawCfg map[string]interface{}) (wallet.Wallet, error) {
	cfg, err := wf.unmarshalConfig(rawCfg)
	if err != nil {
		return nil, err
	}

	return newWallet(cfg)
}

func (wf *ledgerWalletFactory) Remove(name string, rawCfg map[string]interface{}) error {
	return nil
}

func (wf *ledgerWalletFactory) Rename(old, new string, rawCfg map[string]interface{}) error {
	return nil
}

func (wf *ledgerWalletFactory) Import(name string, passphrase string, rawCfg map[string]interface{}, src *wallet.ImportSource) (wallet.Wallet, error) {
	return nil, fmt.Errorf("ledger: import not supported")
}

type ledgerWallet struct {
	cfg        *walletConfig
	signer     *ledgerSigner
	coreSigner *ledgerCoreSigner
}

func newWallet(cfg *walletConfig) (wallet.Wallet, error) {
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

	return &ledgerWallet{
		cfg:        cfg,
		signer:     signer,
		coreSigner: coreSigner,
	}, nil
}

func (w *ledgerWallet) ConsensusSigner() coreSignature.Signer {
	return w.coreSigner
}

func (w *ledgerWallet) Signer() signature.Signer {
	return w.signer
}

func (w *ledgerWallet) Address() types.Address {
	return types.NewAddress(w.SignatureAddressSpec())
}

func (w *ledgerWallet) SignatureAddressSpec() types.SignatureAddressSpec {
	return types.NewSignatureAddressSpecEd25519(w.signer.Public().(ed25519.PublicKey))
}

func (w *ledgerWallet) UnsafeExport() string {
	return ""
}

func init() {
	flags := flag.NewFlagSet("", flag.ContinueOnError)
	flags.String(cfgDerivation, derivationLegacy, "Derivation scheme to use [adr8, legacy]")
	flags.Uint32(cfgNumber, 0, "Key number to use for ADR 0008 key derivation scheme")

	wallet.Register(&ledgerWalletFactory{
		flags: flags,
	})
}
