package file

import (
	"encoding/hex"
	"fmt"
	"strings"

	hdwallet "github.com/miguelmota/go-ethereum-hdwallet"
	"github.com/oasisprotocol/oasis-core/go/common/crypto/signature"
	sdkSignature "github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature/secp256k1"
)

const (
	// privateKeySize is the length of Secp256k1 private key (32 bytes).
	privateKeySize = 32

	// Bip44DerivationPath is the derivation path defined by BIP-44.
	Bip44DerivationPath = "m/44'/60'/0'/0/%d"
)

// Secp256k1FromMnemonic derives a signer using BIP-44 from given mnemonic.
func Secp256k1FromMnemonic(mnemonic string, number uint32) (sdkSignature.Signer, error) {
	wallet, err := hdwallet.NewFromMnemonic(mnemonic)
	if err != nil {
		return nil, fmt.Errorf("failed to parse mnemonic: %w", err)
	}
	path := hdwallet.MustParseDerivationPath(fmt.Sprintf(Bip44DerivationPath, number))
	account, err := wallet.Derive(path, false)
	if err != nil {
		return nil, fmt.Errorf("failed to derive key from mnemonic: %w", err)
	}
	pk, err := wallet.PrivateKeyBytes(account)
	if err != nil {
		return nil, fmt.Errorf("failed to obtain generated private key: %w", err)
	}
	return secp256k1.NewSigner(pk), nil
}

// Secp256k1FromHex creates a signer from given hex-encoded private key.
func Secp256k1FromHex(text string) (sdkSignature.Signer, error) {
	text = strings.TrimPrefix(text, "0x")
	data, err := hex.DecodeString(text)
	if err != nil {
		return nil, err
	}

	if len(data) != privateKeySize {
		return nil, signature.ErrMalformedPrivateKey
	}

	return secp256k1.NewSigner(data), nil
}
