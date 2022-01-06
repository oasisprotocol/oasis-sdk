package file

import (
	"encoding/base64"

	"github.com/oasisprotocol/curve25519-voi/primitives/ed25519"
	"github.com/oasisprotocol/oasis-core/go/common/crypto/signature"
)

// ed25519rawSigner is an in-memory signer that allows deserialization of raw ed25519 keys for use
// in imported wallets that don't use ADR 0008.
type ed25519rawSigner struct {
	privateKey ed25519.PrivateKey
}

func (s *ed25519rawSigner) Public() signature.PublicKey {
	var pk signature.PublicKey
	_ = pk.UnmarshalBinary(s.privateKey.Public().(ed25519.PublicKey))
	return pk
}

func (s *ed25519rawSigner) ContextSign(context signature.Context, message []byte) ([]byte, error) {
	data, err := signature.PrepareSignerMessage(context, message)
	if err != nil {
		return nil, err
	}

	return ed25519.Sign(s.privateKey, data), nil
}

func (s *ed25519rawSigner) String() string {
	return "[redacted private key]"
}

func (s *ed25519rawSigner) Reset() {
	for idx := range s.privateKey {
		s.privateKey[idx] = 0
	}
}

func (s *ed25519rawSigner) unmarshalBase64(text string) error {
	data, err := base64.StdEncoding.DecodeString(text)
	if err != nil {
		return err
	}

	if len(data) != ed25519.PrivateKeySize {
		return signature.ErrMalformedPrivateKey
	}

	s.privateKey = ed25519.PrivateKey(data)
	return nil
}
