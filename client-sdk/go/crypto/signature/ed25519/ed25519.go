package ed25519

import (
	"encoding"
	"encoding/json"

	"github.com/oasisprotocol/oasis-core/go/common/cbor"
	"github.com/oasisprotocol/oasis-core/go/common/crypto/signature"

	sdkSignature "github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature"
)

var (
	_ encoding.BinaryMarshaler   = PublicKey{}
	_ encoding.BinaryUnmarshaler = (*PublicKey)(nil)
	_ encoding.TextMarshaler     = PublicKey{}
	_ encoding.TextUnmarshaler   = (*PublicKey)(nil)
)

// PublicKey is an Ed25519 public key.
type PublicKey signature.PublicKey

type serializedPublicKey struct {
	Ed25519 signature.PublicKey `json:"ed25519"`
}

func (pk PublicKey) MarshalCBOR() ([]byte, error) {
	return cbor.Marshal(serializedPublicKey{Ed25519: signature.PublicKey(pk)}), nil
}

func (pk PublicKey) MarshalJSON() ([]byte, error) {
	return json.Marshal(serializedPublicKey{Ed25519: signature.PublicKey(pk)})
}

// MarshalBinary encodes a public key into binary form.
func (pk PublicKey) MarshalBinary() ([]byte, error) {
	return (signature.PublicKey)(pk).MarshalBinary()
}

// UnMarshalBinary decodes a binary marshaled public key.
func (pk *PublicKey) UnmarshalBinary(data []byte) error {
	return (*signature.PublicKey)(pk).UnmarshalBinary(data)
}

// MarshalText encodes a public key into text form.
func (pk PublicKey) MarshalText() ([]byte, error) {
	return (signature.PublicKey)(pk).MarshalText()
}

// UnmarshalText decodes a text marshaled public key.
func (pk *PublicKey) UnmarshalText(text []byte) error {
	return (*signature.PublicKey)(pk).UnmarshalText(text)
}

// String returns a string representation of the public key.
func (pk PublicKey) String() string {
	return (signature.PublicKey)(pk).String()
}

// Equal compares vs another public key for equality.
func (pk PublicKey) Equal(other sdkSignature.PublicKey) bool {
	opk, ok := other.(PublicKey)
	if !ok {
		return false
	}
	return (signature.PublicKey)(pk).Equal((signature.PublicKey)(opk))
}

// Verify returns true iff the signature is valid for the public key over the context and message.
func (pk PublicKey) Verify(context, message, sig []byte) bool {
	return signature.PublicKey(pk).Verify(signature.Context(context), message, sig)
}

// NewPublicKey creates a new public key from the given Base64 representation or
// panics.
func NewPublicKey(text string) (pk PublicKey) {
	if err := pk.UnmarshalText([]byte(text)); err != nil {
		panic(err)
	}
	return
}

func init() {
	// We need to allow unregistered contexts as contexts may be runtime-dependent.
	signature.UnsafeAllowUnregisteredContexts()
}
