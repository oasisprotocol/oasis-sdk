package types

import (
	"encoding/json"
	"fmt"

	"github.com/oasisprotocol/oasis-core/go/common/cbor"

	"github.com/oasisprotocol/oasis-bridge/client-sdk/go/crypto/signature"
	"github.com/oasisprotocol/oasis-bridge/client-sdk/go/crypto/signature/ed25519"
	"github.com/oasisprotocol/oasis-bridge/client-sdk/go/crypto/signature/secp256k1"
)

var (
	_ json.Marshaler   = (*PublicKey)(nil)
	_ json.Unmarshaler = (*PublicKey)(nil)
	// TODO: CBOR
)

// PublicKey is a serializable public key.
type PublicKey struct {
	signature.PublicKey
}

type serializedPublicKey struct {
	Ed25519   *ed25519.PublicKey   `json:"ed25519,omitempty"`
	Secp256k1 *secp256k1.PublicKey `json:"secp256k1,omitempty"`
}

func (pk *PublicKey) unmarshal(spk *serializedPublicKey) error {
	if spk.Ed25519 != nil && spk.Secp256k1 != nil {
		return fmt.Errorf("malformed public key")
	}

	switch {
	case spk.Ed25519 != nil:
		pk.PublicKey = spk.Ed25519
	case spk.Secp256k1 != nil:
		pk.PublicKey = spk.Secp256k1
	default:
		return fmt.Errorf("unsupported public key type")
	}
	return nil
}

// MarshalCBOR encodes the public key as CBOR.
func (pk *PublicKey) MarshalCBOR() ([]byte, error) {
	return cbor.Marshal(pk.PublicKey), nil
}

// UnmarshalCBOR decodes the public key from CBOR.
func (pk *PublicKey) UnmarshalCBOR(data []byte) error {
	var spk serializedPublicKey
	if err := cbor.Unmarshal(data, &spk); err != nil {
		return err
	}
	return pk.unmarshal(&spk)
}

// MarshalJSON encodes the public key as JSON.
func (pk *PublicKey) MarshalJSON() ([]byte, error) {
	return json.Marshal(pk.PublicKey)
}

// UnmarshalJSON decodes the public key from JSON.
func (pk *PublicKey) UnmarshalJSON(data []byte) error {
	var spk serializedPublicKey
	if err := json.Unmarshal(data, &spk); err != nil {
		return err
	}
	return pk.unmarshal(&spk)
}
