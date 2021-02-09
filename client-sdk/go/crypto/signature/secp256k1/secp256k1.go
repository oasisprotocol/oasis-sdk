package secp256k1

import (
	"encoding/json"

	"github.com/oasisprotocol/oasis-core/go/common/cbor"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature"
)

// PublicKey is a Secp256k1 public key.
type PublicKey []byte

type serializedPublicKey struct {
	Secp256k1 []byte `json:"secp256k1"`
}

func (pk PublicKey) MarshalCBOR() ([]byte, error) {
	return cbor.Marshal(serializedPublicKey{Secp256k1: []byte(pk)}), nil
}

func (pk PublicKey) MarshalJSON() ([]byte, error) {
	return json.Marshal(serializedPublicKey{Secp256k1: []byte(pk)})
}

// MarshalBinary encodes a public key into binary form.
func (pk PublicKey) MarshalBinary() ([]byte, error) {
	panic("not implemented")
}

// UnMarshalBinary decodes a binary marshaled public key.
func (pk *PublicKey) UnmarshalBinary(data []byte) error {
	panic("not implemented")
}

// MarshalText encodes a public key into text form.
func (pk PublicKey) MarshalText() ([]byte, error) {
	panic("not implemented")
}

// UnmarshalText decodes a text marshaled public key.
func (pk *PublicKey) UnmarshalText(text []byte) error {
	panic("not implemented")
}

// String returns a string representation of the public key.
func (pk PublicKey) String() string {
	panic("not implemented")
}

// Equal compares vs another public key for equality.
func (pk PublicKey) Equal(other signature.PublicKey) bool {
	opk, ok := other.(PublicKey)
	if !ok {
		return false
	}
	_ = opk
	panic("not implemented")
}

// Verify returns true iff the signature is valid for the public key over the context and message.
func (pk PublicKey) Verify(context, message, signature []byte) bool {
	// TODO
	return false
}
