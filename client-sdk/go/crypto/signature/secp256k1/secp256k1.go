package secp256k1

import (
	"encoding/base64"
	"encoding/json"

	"github.com/btcsuite/btcd/btcec"

	"github.com/oasisprotocol/oasis-core/go/common/cbor"

	sdkSignature "github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature"
)

// PublicKey is a Secp256k1 public key.
type PublicKey btcec.PublicKey

type serializedPublicKey struct {
	Secp256k1 []byte `json:"secp256k1"`
}

func (pk PublicKey) MarshalCBOR() ([]byte, error) {
	bpk := btcec.PublicKey(pk)
	return cbor.Marshal(serializedPublicKey{Secp256k1: bpk.SerializeCompressed()}), nil
}

func (pk PublicKey) MarshalJSON() ([]byte, error) {
	bpk := btcec.PublicKey(pk)
	return json.Marshal(serializedPublicKey{Secp256k1: bpk.SerializeCompressed()})
}

// MarshalBinary encodes a public key into binary form.
func (pk PublicKey) MarshalBinary() ([]byte, error) {
	bpk := btcec.PublicKey(pk)
	return bpk.SerializeCompressed(), nil
}

// UnMarshalBinary decodes a binary marshaled public key.
func (pk *PublicKey) UnmarshalBinary(data []byte) error {
	parsedPK, err := btcec.ParsePubKey(data, btcec.S256())
	if err != nil {
		return err
	}
	*pk = PublicKey(*parsedPK)
	return nil
}

// MarshalText encodes a public key into text form.
func (pk PublicKey) MarshalText() ([]byte, error) {
	serialized, _ := pk.MarshalBinary()
	return []byte(base64.StdEncoding.EncodeToString(serialized)), nil
}

// UnmarshalText decodes a text marshaled public key.
func (pk *PublicKey) UnmarshalText(text []byte) error {
	decodedPK, err := base64.StdEncoding.DecodeString(string(text))
	if err != nil {
		return err
	}
	return pk.UnmarshalBinary(decodedPK)
}

// String returns a string representation of the public key.
func (pk PublicKey) String() string {
	str, _ := pk.MarshalText()
	return string(str)
}

// Equal compares vs another public key for equality.
func (pk PublicKey) Equal(other sdkSignature.PublicKey) bool {
	opk, ok := other.(PublicKey)
	if !ok {
		return false
	}
	obpk := btcec.PublicKey(opk)
	bpk := btcec.PublicKey(pk)
	return bpk.IsEqual(&obpk)
}

// Verify returns true iff the signature is valid for the public key over the context and message.
func (pk PublicKey) Verify(context, message, signature []byte) bool {
	sig, err := btcec.ParseSignature(signature, btcec.S256())
	if err != nil {
		return false
	}
	data, err := PrepareSignerMessage(sdkSignature.Context(context), message)
	if err != nil {
		return false
	}
	bpk := btcec.PublicKey(pk)
	return sig.Verify(data, &bpk)
}
