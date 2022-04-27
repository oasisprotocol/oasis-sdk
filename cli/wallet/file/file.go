package file

import (
	"crypto/rand"
	"encoding/base64"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io/ioutil"
	"os"
	"path/filepath"
	"strings"

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
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature/secp256k1"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

const (
	// Kind is the account kind for the file-backed accounts.
	Kind = "file"

	cfgAlgorithm = "file.algorithm"
	cfgNumber    = "file.number"

	stateKeySize   = 32
	stateNonceSize = 32
	kdfSaltSize    = 32
)

// SupportedAlgorithmsForImport returns the algorithms supported by the given import kind.
func SupportedAlgorithmsForImport(kind *wallet.ImportKind) []string {
	if kind == nil {
		return []string{wallet.AlgorithmEd25519Adr8, wallet.AlgorithmEd25519Raw, wallet.AlgorithmSecp256k1Bip44, wallet.AlgorithmSecp256k1Raw}
	}

	switch *kind {
	case wallet.ImportKindMnemonic:
		return []string{wallet.AlgorithmEd25519Adr8, wallet.AlgorithmSecp256k1Bip44}
	case wallet.ImportKindPrivateKey:
		return []string{wallet.AlgorithmEd25519Raw, wallet.AlgorithmSecp256k1Raw}
	default:
		return []string{}
	}
}

type accountConfig struct {
	Algorithm string `mapstructure:"algorithm"`
	Number    uint32 `mapstructure:"number,omitempty"`
}

type secretState struct {
	// Algorithm is the cryptographic algorithm used by the account.
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

func getAccountFilename(name string) string {
	return filepath.Join(config.Directory(), fmt.Sprintf("%s.wallet", name))
}

type fileAccountFactory struct {
	flags *flag.FlagSet
}

func (af *fileAccountFactory) Kind() string {
	return Kind
}

func (af *fileAccountFactory) PrettyKind(rawCfg map[string]interface{}) string {
	cfg, err := af.unmarshalConfig(rawCfg)
	if err != nil {
		return ""
	}

	// In case of ADR8 or BIP44 show the keypair number.
	var number string
	switch cfg.Algorithm {
	case wallet.AlgorithmEd25519Adr8, wallet.AlgorithmSecp256k1Bip44:
		number = fmt.Sprintf(":%d", cfg.Number)
	}
	return fmt.Sprintf("%s (%s%s)", Kind, cfg.Algorithm, number)
}

func (af *fileAccountFactory) Flags() *flag.FlagSet {
	return af.flags
}

func (af *fileAccountFactory) GetConfigFromFlags() (map[string]interface{}, error) {
	cfg := make(map[string]interface{})
	cfg["algorithm"], _ = af.flags.GetString(cfgAlgorithm)
	cfg["number"], _ = af.flags.GetUint32(cfgNumber)
	return cfg, nil
}

func (af *fileAccountFactory) GetConfigFromSurvey(kind *wallet.ImportKind) (map[string]interface{}, error) {
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
		"algorithm": answers.Algorithm,
		"number":    answers.Number,
	}, nil
}

func (af *fileAccountFactory) DataPrompt(kind wallet.ImportKind, rawCfg map[string]interface{}) survey.Prompt {
	switch kind {
	case wallet.ImportKindMnemonic:
		return &survey.Multiline{Message: "Mnemonic:"}
	case wallet.ImportKindPrivateKey:
		cfg, err := af.unmarshalConfig(rawCfg)
		if err != nil {
			return nil
		}
		switch cfg.Algorithm {
		case wallet.AlgorithmEd25519Raw:
			return &survey.Multiline{Message: "Private key (base64-encoded):"}
		case wallet.AlgorithmSecp256k1Raw:
			return &survey.Multiline{Message: "Private key (hex-encoded):"}
		default:
			return nil
		}
	default:
		return nil
	}
}

func (af *fileAccountFactory) DataValidator(kind wallet.ImportKind, rawCfg map[string]interface{}) survey.Validator {
	return func(ans interface{}) error {
		switch kind {
		case wallet.ImportKindMnemonic:
		case wallet.ImportKindPrivateKey:
			cfg, err := af.unmarshalConfig(rawCfg)
			if err != nil {
				return nil
			}
			switch cfg.Algorithm {
			case wallet.AlgorithmEd25519Raw:
				// Ensure the private key is base64 encoded.
				_, err := base64.StdEncoding.DecodeString(ans.(string))
				if err != nil {
					return fmt.Errorf("private key must be base64-encoded: %w", err)
				}
			case wallet.AlgorithmSecp256k1Raw:
				// Ensure the private key is hex encoded.
				a := strings.TrimPrefix(ans.(string), "0x")
				_, err := hex.DecodeString(a)
				if err != nil {
					return fmt.Errorf("private key must be hex-encoded: %w", err)
				}
			default:
				return fmt.Errorf("unsupported algorithm for %s: %s", wallet.ImportKindPrivateKey, cfg.Algorithm)
			}
		default:
			return fmt.Errorf("unsupported import kind: %s", kind)
		}
		return nil
	}
}

func (af *fileAccountFactory) RequiresPassphrase() bool {
	// A file-backed account always requires a passphrase.
	return true
}

func (af *fileAccountFactory) SupportedImportKinds() []wallet.ImportKind {
	return []wallet.ImportKind{
		wallet.ImportKindMnemonic,
		wallet.ImportKindPrivateKey,
	}
}

func (af *fileAccountFactory) HasConsensusSigner(rawCfg map[string]interface{}) bool {
	cfg, err := af.unmarshalConfig(rawCfg)
	if err != nil {
		return false
	}

	switch cfg.Algorithm {
	case wallet.AlgorithmEd25519Raw, wallet.AlgorithmEd25519Adr8:
		return true
	}
	return false
}

func (af *fileAccountFactory) unmarshalConfig(raw map[string]interface{}) (*accountConfig, error) {
	if raw == nil {
		return nil, fmt.Errorf("missing configuration")
	}

	var cfg accountConfig
	if err := mapstructure.Decode(raw, &cfg); err != nil {
		return nil, err
	}
	return &cfg, nil
}

func (af *fileAccountFactory) Create(name string, passphrase string, rawCfg map[string]interface{}) (wallet.Account, error) {
	cfg, err := af.unmarshalConfig(rawCfg)
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
	if err := ioutil.WriteFile(getAccountFilename(name), raw, 0o600); err != nil {
		return nil, fmt.Errorf("failed to save state: %w", err)
	}

	// Create a proper account based on the chosen algorithm.
	return newAccount(state, cfg)
}

func (af *fileAccountFactory) Load(name string, passphrase string, rawCfg map[string]interface{}) (wallet.Account, error) {
	cfg, err := af.unmarshalConfig(rawCfg)
	if err != nil {
		return nil, err
	}

	// Load state from encrypted file.
	raw, err := ioutil.ReadFile(getAccountFilename(name))
	if err != nil {
		return nil, fmt.Errorf("failed to load account state: %w", err)
	}

	var envelope secretStateEnvelope
	if err = json.Unmarshal(raw, &envelope); err != nil {
		return nil, fmt.Errorf("failed to load account state: %w", err)
	}

	var state *secretState
	if state, err = envelope.Open(passphrase); err != nil {
		return nil, fmt.Errorf("failed to open account state (maybe incorrect passphrase?)")
	}

	return newAccount(state, cfg)
}

func (af *fileAccountFactory) Remove(name string, rawCfg map[string]interface{}) error {
	return os.Remove(getAccountFilename(name))
}

func (af *fileAccountFactory) Rename(old, new string, rawCfg map[string]interface{}) error {
	return os.Rename(getAccountFilename(old), getAccountFilename(new))
}

func (af *fileAccountFactory) Import(name string, passphrase string, rawCfg map[string]interface{}, src *wallet.ImportSource) (wallet.Account, error) {
	cfg, err := af.unmarshalConfig(rawCfg)
	if err != nil {
		return nil, err
	}

	// Validate compatibility of algorithm and import source.
	switch src.Kind {
	case wallet.ImportKindMnemonic:
		switch cfg.Algorithm {
		case wallet.AlgorithmEd25519Adr8, wallet.AlgorithmSecp256k1Bip44:
		default:
			return nil, fmt.Errorf("algorithm '%s' does not support import from mnemonic", cfg.Algorithm)
		}
	case wallet.ImportKindPrivateKey:
		switch cfg.Algorithm {
		case wallet.AlgorithmEd25519Raw, wallet.AlgorithmSecp256k1Raw:
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

	// Create a proper account based on the chosen algorithm.
	acc, err := newAccount(&state, cfg)
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
	if err := ioutil.WriteFile(getAccountFilename(name), raw, 0o600); err != nil {
		return nil, fmt.Errorf("failed to save state: %w", err)
	}
	return acc, nil
}

type fileAccount struct {
	cfg    *accountConfig
	state  *secretState
	signer signature.Signer
}

func newAccount(state *secretState, cfg *accountConfig) (wallet.Account, error) {
	switch state.Algorithm {
	case wallet.AlgorithmEd25519Adr8:
		// For Ed25519 use the ADR 0008 derivation scheme.
		signer, _, err := sakg.GetAccountSigner(state.Data, "", cfg.Number)
		if err != nil {
			return nil, fmt.Errorf("failed to derive signer: %w", err)
		}

		return &fileAccount{
			cfg:    cfg,
			state:  state,
			signer: ed25519.WrapSigner(signer),
		}, nil
	case wallet.AlgorithmEd25519Raw:
		// For Ed25519-Raw use the raw private key.
		var signer ed25519rawSigner
		if err := signer.unmarshalBase64(state.Data); err != nil {
			return nil, fmt.Errorf("failed to initialize signer: %w", err)
		}

		return &fileAccount{
			cfg:    cfg,
			state:  state,
			signer: ed25519.WrapSigner(&signer),
		}, nil
	case wallet.AlgorithmSecp256k1Bip44:
		// For Secp256k1-BIP-44 use the BIP-44 derivation scheme.
		signer, err := Secp256k1FromMnemonic(state.Data, cfg.Number)
		if err != nil {
			return nil, fmt.Errorf("failed to initialize signer: %w", err)
		}
		return &fileAccount{
			cfg:    cfg,
			state:  state,
			signer: signer,
		}, nil
	case wallet.AlgorithmSecp256k1Raw:
		// For Secp256k1-Raw use the raw private key.
		signer, err := Secp256k1FromHex(state.Data)
		if err != nil {
			return nil, fmt.Errorf("failed to initialize signer: %w", err)
		}

		return &fileAccount{
			cfg:    cfg,
			state:  state,
			signer: signer,
		}, nil
	default:
		return nil, fmt.Errorf("algorithm '%s' not supported", state.Algorithm)
	}
}

func (a *fileAccount) ConsensusSigner() coreSignature.Signer {
	type wrappedSigner interface {
		Unwrap() coreSignature.Signer
	}

	if ws, ok := a.signer.(wrappedSigner); ok {
		return ws.Unwrap()
	}
	return nil
}

func (a *fileAccount) Signer() signature.Signer {
	return a.signer
}

func (a *fileAccount) Address() types.Address {
	return types.NewAddress(a.SignatureAddressSpec())
}

func (a *fileAccount) SignatureAddressSpec() types.SignatureAddressSpec {
	switch a.cfg.Algorithm {
	case wallet.AlgorithmEd25519Adr8, wallet.AlgorithmEd25519Raw:
		return types.NewSignatureAddressSpecEd25519(a.Signer().Public().(ed25519.PublicKey))
	case wallet.AlgorithmSecp256k1Bip44, wallet.AlgorithmSecp256k1Raw:
		return types.NewSignatureAddressSpecSecp256k1Eth(a.Signer().Public().(secp256k1.PublicKey))
	default:
		return types.SignatureAddressSpec{}
	}
}

func (a *fileAccount) UnsafeExport() string {
	return a.state.Data
}

func init() {
	flags := flag.NewFlagSet("", flag.ContinueOnError)
	flags.String(cfgAlgorithm, wallet.AlgorithmEd25519Adr8, fmt.Sprintf("Cryptographic algorithm to use for this account [%s, %s]", wallet.AlgorithmEd25519Adr8, wallet.AlgorithmSecp256k1Bip44))
	flags.Uint32(cfgNumber, 0, "Key number to use in the key derivation scheme")

	wallet.Register(&fileAccountFactory{
		flags: flags,
	})
}
