package types

import (
	"encoding/json"
	"fmt"
	"math/bits"

	"github.com/oasisprotocol/oasis-core/go/common/cbor"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature/ed25519"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature/secp256k1"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature/sr25519"
)

var (
	_ json.Marshaler   = (*PublicKey)(nil)
	_ json.Unmarshaler = (*PublicKey)(nil)
	_ cbor.Marshaler   = (*PublicKey)(nil)
	_ cbor.Unmarshaler = (*PublicKey)(nil)
)

// PublicKey is a serializable public key.
type PublicKey struct {
	signature.PublicKey
}

type serializedPublicKey struct {
	Ed25519   *ed25519.PublicKey   `json:"ed25519,omitempty"`
	Secp256k1 *secp256k1.PublicKey `json:"secp256k1,omitempty"`
	Sr25519   *sr25519.PublicKey   `json:"sr25519,omitempty"`
}

func (pk *PublicKey) marshal() (*serializedPublicKey, error) {
	var spk serializedPublicKey
	switch inner := pk.PublicKey.(type) {
	case ed25519.PublicKey:
		spk.Ed25519 = &inner
	case secp256k1.PublicKey:
		spk.Secp256k1 = &inner
	case sr25519.PublicKey:
		spk.Sr25519 = &inner
	default:
		return nil, fmt.Errorf("unsupported public key type")
	}
	return &spk, nil
}

func (pk *PublicKey) unmarshal(spk *serializedPublicKey) error {
	if spk.Ed25519 != nil && spk.Secp256k1 != nil && spk.Sr25519 != nil {
		return fmt.Errorf("malformed public key")
	}

	switch {
	case spk.Ed25519 != nil:
		pk.PublicKey = spk.Ed25519
	case spk.Secp256k1 != nil:
		pk.PublicKey = spk.Secp256k1
	case spk.Sr25519 != nil:
		pk.PublicKey = spk.Sr25519
	default:
		return fmt.Errorf("unsupported public key type")
	}
	return nil
}

// MarshalCBOR encodes the public key as CBOR.
func (pk *PublicKey) MarshalCBOR() ([]byte, error) {
	spk, err := pk.marshal()
	if err != nil {
		return nil, err
	}
	return cbor.Marshal(spk), nil
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
	spk, err := pk.marshal()
	if err != nil {
		return nil, err
	}
	return json.Marshal(spk)
}

// UnmarshalJSON decodes the public key from JSON.
func (pk *PublicKey) UnmarshalJSON(data []byte) error {
	var spk serializedPublicKey
	if err := json.Unmarshal(data, &spk); err != nil {
		return err
	}
	return pk.unmarshal(&spk)
}

// MultisigSigner is one of the signers in a multisig configuration.
type MultisigSigner struct {
	PublicKey PublicKey `json:"public_key"`
	Weight    uint64    `json:"weight"`
}

// MultisigConfig is a multisig configuration.
// A set of signers with total "weight" greater than or equal to a "threshold" can authenticate
// for the configuration.
type MultisigConfig struct {
	Signers   []MultisigSigner `json:"signers"`
	Threshold uint64           `json:"threshold"`
}

// ValidateBasic performs some sanity checks. This looks at the configuration only. There is no cryptographic
// verification of any signatures.
func (mc *MultisigConfig) ValidateBasic() error {
	if mc.Threshold == 0 {
		return fmt.Errorf("zero threshold")
	}
	var total uint64
	encounteredKeys := make(map[PublicKey]bool)
	for i, signer := range mc.Signers {
		if encounteredKeys[signer.PublicKey] {
			return fmt.Errorf("signer %d duplicated", i)
		}
		encounteredKeys[signer.PublicKey] = true
		if signer.Weight == 0 {
			return fmt.Errorf("signer %d zero weight", i)
		}
		newTotal, carry := bits.Add64(total, signer.Weight, 0)
		if carry != 0 {
			return fmt.Errorf("weight overflow")
		}
		total = newTotal
	}
	if total < mc.Threshold {
		return fmt.Errorf("impossible threshold")
	}
	return nil
}

// Batch checks that enough signers have signed and returns vectors of public keys and signatures
// for batch verification of those signatures. This internally calls `ValidateBasic`.
func (mc *MultisigConfig) Batch(signatureSet [][]byte) ([]PublicKey, [][]byte, error) {
	if err := mc.ValidateBasic(); err != nil {
		return nil, nil, err
	}
	if len(signatureSet) != len(mc.Signers) {
		return nil, nil, fmt.Errorf("mismatched signature set length")
	}
	var total uint64
	var publicKeys []PublicKey
	var signatures [][]byte
	for i := 0; i < len(mc.Signers); i++ {
		if signatureSet[i] != nil {
			total += mc.Signers[i].Weight
			publicKeys = append(publicKeys, mc.Signers[i].PublicKey)
			signatures = append(signatures, signatureSet[i])
		}
	}
	if total < mc.Threshold {
		return nil, nil, fmt.Errorf("insufficient weight")
	}
	return publicKeys, signatures, nil
}
