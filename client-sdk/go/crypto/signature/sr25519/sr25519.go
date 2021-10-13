package sr25519

import (
	"crypto/sha512"
	"encoding/base64"

	"github.com/oasisprotocol/curve25519-voi/primitives/sr25519"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature"
)

// PublicKey is an Sr25519 public key.
type PublicKey struct {
	inner *sr25519.PublicKey
}

// MarshalBinary encodes a public key into binary form.
func (pk PublicKey) MarshalBinary() ([]byte, error) {
	if pk.inner == nil {
		return nil, nil
	}

	return pk.inner.MarshalBinary()
}

// UnmarshalBinary decodes a binary marshaled public key.
func (pk *PublicKey) UnmarshalBinary(data []byte) error {
	var err error
	if pk.inner, err = sr25519.NewPublicKeyFromBytes(data); err != nil {
		return err
	}
	return nil
}

// MarshalText encodes a public key into text form.
func (pk PublicKey) MarshalText() ([]byte, error) {
	b, err := pk.MarshalBinary()
	if err != nil {
		return nil, err
	}

	return []byte(base64.StdEncoding.EncodeToString(b)), nil
}

// UnmarshalText decodes a text marshaled public key.
func (pk *PublicKey) UnmarshalText(text []byte) error {
	b, err := base64.StdEncoding.DecodeString(string(text))
	if err != nil {
		return err
	}
	return pk.UnmarshalBinary(b)
}

// String returns a string representation of the public key.
func (pk PublicKey) String() string {
	s, _ := pk.MarshalText()
	return string(s)
}

// Equal compares vs another public key for equality.
func (pk PublicKey) Equal(other signature.PublicKey) bool {
	opk, ok := other.(PublicKey)
	if !ok {
		return false
	}
	if pk.inner == nil && opk.inner != nil || pk.inner != nil && opk.inner == nil {
		return false
	}

	return pk.inner.Equal(opk.inner)
}

// Verify returns true iff the signature is valid for the public key
// over the context and message.
func (pk PublicKey) Verify(context, message, signature []byte) bool {
	transcript := newSigningTranscript(context, message)

	srSignature, err := sr25519.NewSignatureFromBytes(signature)
	if err != nil {
		return false
	}

	return pk.inner.Verify(transcript, srSignature)
}

// Because Sr25519 is actually nice, and we don't need to care about "muh
// ledger", use the native domain separation and pre-hashed message support.
//
// This also means that anyone that wants to implement "PrepareSignerMessage"
// is "doing it wrong".
func newSigningTranscript(context, message []byte) *sr25519.SigningTranscript {
	signingContext := sr25519.NewSigningContext(context)
	h := sha512.New512_256()
	_, _ = h.Write(message)

	return signingContext.NewTranscriptHash(h)
}
