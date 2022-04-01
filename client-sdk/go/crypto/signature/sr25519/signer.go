package sr25519

import (
	"fmt"

	"github.com/oasisprotocol/curve25519-voi/primitives/sr25519"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature"
)

type signer struct {
	keyPair *sr25519.KeyPair
}

func (s *signer) Public() signature.PublicKey {
	return PublicKey{
		inner: s.keyPair.PublicKey(),
	}
}

func (s *signer) ContextSign(context, message []byte) ([]byte, error) {
	transcript := newSigningTranscript(context, message)

	sig, err := s.keyPair.Sign(nil, transcript)
	if err != nil {
		return nil, err
	}

	return sig.MarshalBinary()
}

func (s *signer) Sign(message []byte) ([]byte, error) {
	return nil, fmt.Errorf("sr25519: signing without context not implemented")
}

func (s *signer) String() string {
	return "sr25519 signer: " + s.Public().String()
}

func (s *signer) Reset() {
	// curve25519-voi acknowledges that memory sanitization in Go is
	// a totally lost cause.
	s.keyPair = nil
}

// NewSigner creates a new Sr25519 signer using the given byte-serialized
// private key.
func NewSigner(b []byte) (signature.Signer, error) {
	secretKey, err := sr25519.NewSecretKeyFromBytes(b)
	if err != nil {
		return nil, err
	}

	return NewSignerFromKeyPair(secretKey.KeyPair()), nil
}

// NewSignerFromKeyPair creates a new Sr25519 signer using the given key pair.
func NewSignerFromKeyPair(kp *sr25519.KeyPair) signature.Signer {
	return &signer{kp}
}
