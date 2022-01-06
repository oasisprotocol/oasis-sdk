package file

import (
	"crypto/rand"
	"encoding/json"
	"fmt"
	"io/ioutil"
	"os"
	"path/filepath"

	"github.com/AlecAivazis/survey/v2"
	"github.com/mitchellh/mapstructure"
	flag "github.com/spf13/pflag"
	bip39 "github.com/tyler-smith/go-bip39"
	"golang.org/x/crypto/argon2"

	"github.com/oasisprotocol/deoxysii"
	"github.com/oasisprotocol/oasis-core/go/common/crypto/sakg"
	coreSignature "github.com/oasisprotocol/oasis-core/go/common/crypto/signature"

	"github.com/oasisprotocol/oasis-sdk/cli/config"
	"github.com/oasisprotocol/oasis-sdk/cli/wallet"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature/ed25519"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

const (
	// Kind is the wallet kind for the file-backed wallet.
	Kind = "file"

	cfgAlgorithm = "algorithm"
	cfgNumber    = "number"

	// AlgorithmEd25519Adr8 is the Ed25519 algorithm using the ADR-0008 derivation.
	AlgorithmEd25519Adr8 = "ed25519-adr8"
	// AlgorithmEd25519Raw is the Ed25519 algorithm using raw private keys.
	AlgorithmEd25519Raw = "ed25519-raw"

	stateKeySize   = 32
	stateNonceSize = 32
	kdfSaltSize    = 32
)

// SupportedAlgorithmsForImport returns the algorithms supported by the given import kind.
func SupportedAlgorithmsForImport(kind *wallet.ImportKind) []string {
	if kind == nil {
		return []string{AlgorithmEd25519Adr8, AlgorithmEd25519Raw}
	}

	switch *kind {
	case wallet.ImportKindMnemonic:
		return []string{AlgorithmEd25519Adr8}
	case wallet.ImportKindPrivateKey:
		return []string{AlgorithmEd25519Raw}
	default:
		return []string{}
	}
}

type walletConfig struct {
	Algorithm string `mapstructure:"algorithm"`
	Number    uint32 `mapstructure:"number,omitempty"`
}

type secretState struct {
	// Algorithm is the cryptographic algorithm used by the wallet.
	Algorithm string `json:"algorithm"`

	// Data is the secret data used to derive the private key.
	Data string `json:"data"`
}

func (s *secretState) Seal(passphrase string) (*secretStateEnvelope, error) {
	var nonce [stateNonceSize]byte
	_, err := rand.Read(nonce[:])
	if err != nil {
		return nil, err
	}

	var salt [kdfSaltSize]byte
	_, err = rand.Read(salt[:])
	if err != nil {
		return nil, err
	}

	envelope := &secretStateEnvelope{
		KDF: secretStateKDF{
			Argon2: &kdfArgon2{
				Salt:    salt[:],
				Time:    1,
				Memory:  64 * 1024,
				Threads: 4,
			},
		},
		Nonce: nonce[:],
	}
	key, err := envelope.deriveKey(passphrase)
	if err != nil {
		return nil, err
	}

	data, err := json.Marshal(s)
	if err != nil {
		return nil, err
	}

	// Initialize a Deoxys-II instance with the provided key and encrypt.
	aead, err := deoxysii.New(key)
	if err != nil {
		return nil, err
	}
	envelope.Data = aead.Seal(nil, envelope.Nonce[:aead.NonceSize()], data, nil)

	return envelope, nil
}

type secretStateEnvelope struct {
	KDF   secretStateKDF `json:"kdf"`
	Nonce []byte         `json:"nonce"`
	Data  []byte         `json:"data"`
}

type secretStateKDF struct {
	Argon2 *kdfArgon2 `json:"argon2,omitempty"`
}

type kdfArgon2 struct {
	Salt    []byte `json:"salt"`
	Time    uint32 `json:"time"`
	Memory  uint32 `json:"memory"`
	Threads uint8  `json:"threads"`
}

func (k *kdfArgon2) deriveKey(passphrase string) ([]byte, error) {
	return argon2.IDKey([]byte(passphrase), k.Salt, k.Time, k.Memory, k.Threads, stateKeySize), nil
}

func (e *secretStateEnvelope) deriveKey(passphrase string) ([]byte, error) {
	switch {
	case e.KDF.Argon2 != nil:
		return e.KDF.Argon2.deriveKey(passphrase)
	default:
		return nil, fmt.Errorf("unsupported key derivation algorithm")
	}
}

func (e *secretStateEnvelope) Open(passphrase string) (*secretState, error) {
	// Derive key.
	key, err := e.deriveKey(passphrase)
	if err != nil {
		return nil, err
	}

	// Initialize a Deoxys-II instance with the provided key and decrypt.
	aead, err := deoxysii.New(key)
	if err != nil {
		return nil, err
	}
	pt, err := aead.Open(nil, e.Nonce[:aead.NonceSize()], e.Data, nil)
	if err != nil {
		return nil, err
	}

	// Deserialize the inner state.
	var state secretState
	if err := json.Unmarshal(pt, &state); err != nil {
		return nil, err
	}

	return &state, nil
}

func getWalletFilename(name string) string {
	return filepath.Join(config.Directory(), fmt.Sprintf("%s.wallet", name))
}

type fileWalletFactory struct {
	flags *flag.FlagSet
}

func (wf *fileWalletFactory) Kind() string {
	return Kind
}

func (wf *fileWalletFactory) Flags() *flag.FlagSet {
	return wf.flags
}

func (wf *fileWalletFactory) GetConfigFromFlags() (map[string]interface{}, error) {
	cfg := make(map[string]interface{})
	cfg[cfgAlgorithm], _ = wf.flags.GetString(cfgAlgorithm)
	cfg[cfgNumber], _ = wf.flags.GetUint32(cfgNumber)
	return cfg, nil
}

func (wf *fileWalletFactory) GetConfigFromSurvey(kind *wallet.ImportKind) (map[string]interface{}, error) {
	// Ask for import details.
	var answers struct {
		Algorithm string
		Number    uint32
	}
	questions := []*survey.Question{
		{
			Name: "algorithm",
			Prompt: &survey.Select{
				Message: "Algorithm:",
				Options: SupportedAlgorithmsForImport(kind),
			},
		},
	}
	if kind != nil && *kind == wallet.ImportKindMnemonic {
		questions = append(questions, &survey.Question{
			Name: "number",
			Prompt: &survey.Input{
				Message: "Key number:",
				Default: "0",
			},
		})
	}
	err := survey.Ask(questions, &answers)
	if err != nil {
		return nil, err
	}

	return map[string]interface{}{
		cfgAlgorithm: answers.Algorithm,
		cfgNumber:    answers.Number,
	}, nil
}

func (wf *fileWalletFactory) RequiresPassphrase() bool {
	// A file-backed wallet always requires a passphrase.
	return true
}

func (wf *fileWalletFactory) SupportedImportKinds() []wallet.ImportKind {
	return []wallet.ImportKind{
		wallet.ImportKindMnemonic,
		wallet.ImportKindPrivateKey,
	}
}

func (wf *fileWalletFactory) unmarshalConfig(raw map[string]interface{}) (*walletConfig, error) {
	if raw == nil {
		return nil, fmt.Errorf("missing configuration")
	}

	var cfg walletConfig
	if err := mapstructure.Decode(raw, &cfg); err != nil {
		return nil, err
	}
	return &cfg, nil
}

func (wf *fileWalletFactory) Create(name string, passphrase string, rawCfg map[string]interface{}) (wallet.Wallet, error) {
	cfg, err := wf.unmarshalConfig(rawCfg)
	if err != nil {
		return nil, err
	}

	// Generate entropy.
	entropy, err := bip39.NewEntropy(256)
	if err != nil {
		return nil, err
	}
	mnemonic, err := bip39.NewMnemonic(entropy)
	if err != nil {
		return nil, err
	}
	state := &secretState{
		Algorithm: cfg.Algorithm,
		Data:      mnemonic,
	}

	// Seal state.
	envelope, err := state.Seal(passphrase)
	if err != nil {
		return nil, fmt.Errorf("failed to seal state: %w", err)
	}

	raw, err := json.Marshal(envelope)
	if err != nil {
		return nil, fmt.Errorf("failed to marshal envelope: %w", err)
	}
	if err := ioutil.WriteFile(getWalletFilename(name), raw, 0o600); err != nil {
		return nil, fmt.Errorf("failed to save state: %w", err)
	}

	// Create a proper wallet based on the chosen algorithm.
	return newWallet(state, cfg)
}

func (wf *fileWalletFactory) Load(name string, passphrase string, rawCfg map[string]interface{}) (wallet.Wallet, error) {
	cfg, err := wf.unmarshalConfig(rawCfg)
	if err != nil {
		return nil, err
	}

	// Load state from encrypted file.
	raw, err := ioutil.ReadFile(getWalletFilename(name))
	if err != nil {
		return nil, fmt.Errorf("failed to load wallet state: %w", err)
	}

	var envelope secretStateEnvelope
	if err = json.Unmarshal(raw, &envelope); err != nil {
		return nil, fmt.Errorf("failed to load wallet state: %w", err)
	}

	var state *secretState
	if state, err = envelope.Open(passphrase); err != nil {
		return nil, fmt.Errorf("failed to open wallet state (maybe incorrect passphrase?)")
	}

	return newWallet(state, cfg)
}

func (wf *fileWalletFactory) Remove(name string, rawCfg map[string]interface{}) error {
	return os.Remove(getWalletFilename(name))
}

func (wf *fileWalletFactory) Import(name string, passphrase string, rawCfg map[string]interface{}, src *wallet.ImportSource) (wallet.Wallet, error) {
	cfg, err := wf.unmarshalConfig(rawCfg)
	if err != nil {
		return nil, err
	}

	// Validate compatibility of algorithm and import source.
	switch src.Kind {
	case wallet.ImportKindMnemonic:
		switch cfg.Algorithm {
		case AlgorithmEd25519Adr8:
		default:
			return nil, fmt.Errorf("algorithm '%s' does not support import from mnemonic", cfg.Algorithm)
		}
	case wallet.ImportKindPrivateKey:
		switch cfg.Algorithm {
		case AlgorithmEd25519Raw:
		default:
			return nil, fmt.Errorf("algorithm '%s' does not support import from private key", cfg.Algorithm)
		}
	default:
		return nil, fmt.Errorf("unsupported import kind: %s", src.Kind)
	}

	state := secretState{
		Algorithm: cfg.Algorithm,
		Data:      src.Data,
	}

	// Create a proper wallet based on the chosen algorithm.
	wallet, err := newWallet(&state, cfg)
	if err != nil {
		return nil, err
	}

	// Seal state.
	envelope, err := state.Seal(passphrase)
	if err != nil {
		return nil, fmt.Errorf("failed to seal state: %w", err)
	}

	raw, err := json.Marshal(envelope)
	if err != nil {
		return nil, fmt.Errorf("failed to marshal envelope: %w", err)
	}
	if err := ioutil.WriteFile(getWalletFilename(name), raw, 0o600); err != nil {
		return nil, fmt.Errorf("failed to save state: %w", err)
	}
	return wallet, nil
}

type fileWallet struct {
	cfg    *walletConfig
	state  *secretState
	signer signature.Signer
}

func newWallet(state *secretState, cfg *walletConfig) (wallet.Wallet, error) {
	switch state.Algorithm {
	case AlgorithmEd25519Adr8:
		// For Ed25519 use the ADR 0008 derivation scheme.
		signer, _, err := sakg.GetAccountSigner(state.Data, "", cfg.Number)
		if err != nil {
			return nil, fmt.Errorf("failed to derive signer: %w", err)
		}

		return &fileWallet{
			cfg:    cfg,
			state:  state,
			signer: ed25519.WrapSigner(signer),
		}, nil
	case AlgorithmEd25519Raw:
		// For Ed25519-Raw use the raw private key.
		var signer ed25519rawSigner
		if err := signer.unmarshalBase64(state.Data); err != nil {
			return nil, fmt.Errorf("failed to initialize signer: %w", err)
		}

		return &fileWallet{
			cfg:    cfg,
			state:  state,
			signer: ed25519.WrapSigner(&signer),
		}, nil
	default:
		return nil, fmt.Errorf("algorithm '%s' not supported", state.Algorithm)
	}
}

func (w *fileWallet) ConsensusSigner() coreSignature.Signer {
	type wrappedSigner interface {
		Unwrap() coreSignature.Signer
	}

	if ws, ok := w.signer.(wrappedSigner); ok {
		return ws.Unwrap()
	}
	return nil
}

func (w *fileWallet) Signer() signature.Signer {
	return w.signer
}

func (w *fileWallet) Address() types.Address {
	return types.NewAddress(w.SignatureAddressSpec())
}

func (w *fileWallet) SignatureAddressSpec() types.SignatureAddressSpec {
	switch w.cfg.Algorithm {
	case AlgorithmEd25519Adr8, AlgorithmEd25519Raw:
		return types.NewSignatureAddressSpecEd25519(w.Signer().Public().(ed25519.PublicKey))
	default:
		return types.SignatureAddressSpec{}
	}
}

func (w *fileWallet) UnsafeExport() string {
	return w.state.Data
}

func init() {
	flags := flag.NewFlagSet("", flag.ContinueOnError)
	flags.String(cfgAlgorithm, AlgorithmEd25519Adr8, "Cryptographic algorithm to use for this wallet")
	flags.Uint32(cfgNumber, 0, "Key number to use for ADR 0008 key derivation scheme")

	wallet.Register(&fileWalletFactory{
		flags: flags,
	})
}
