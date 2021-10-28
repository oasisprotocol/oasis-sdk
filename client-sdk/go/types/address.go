package types

import (
	"encoding"

	"golang.org/x/crypto/sha3"

	"github.com/oasisprotocol/oasis-core/go/common/cbor"
	"github.com/oasisprotocol/oasis-core/go/common/crypto/address"
	"github.com/oasisprotocol/oasis-core/go/common/encoding/bech32"
	staking "github.com/oasisprotocol/oasis-core/go/staking/api"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature/ed25519"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature/secp256k1"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature/sr25519"
)

var (
	// AddressV0Ed25519Context is the unique context for v0 Ed25519-based addresses.
	// It is shared with the consensus layer addresses on purpose.
	AddressV0Ed25519Context = staking.AddressV0Context
	// AddressV0Secp256k1EthContext is the unique context for v0 secp256k1-based addresses.
	AddressV0Secp256k1EthContext = address.NewContext("oasis-runtime-sdk/address: secp256k1eth", 0)
	// AddressV0Sr25519Context is the unique context for v0 Sr25519-based addresses.
	AddressV0Sr25519Context = address.NewContext("oasis-runtime-sdk/address: sr25519", 0)
	// AddressV0MultisigContext is the unique context for v0 multisig addresses.
	AddressV0MultisigContext = address.NewContext("oasis-runtime-sdk/address: multisig", 0)
	// AddressV0ModuleContext is the unique context for v0 module addresses.
	AddressV0ModuleContext = address.NewContext("oasis-runtime-sdk/address: module", 0)
	// AddressBech32HRP is the unique human readable part of Bech32 encoded
	// staking account addresses.
	AddressBech32HRP = staking.AddressBech32HRP

	_ encoding.BinaryMarshaler   = Address{}
	_ encoding.BinaryUnmarshaler = (*Address)(nil)
	_ encoding.TextMarshaler     = Address{}
	_ encoding.TextUnmarshaler   = (*Address)(nil)
)

// SignatureAddressSpec is information for signature-based authentication and public key-based
// address derivation.
type SignatureAddressSpec struct {
	// Ed25519 address derivation compatible with the consensus layer.
	Ed25519 *ed25519.PublicKey `json:"ed25519,omitempty"`

	// Secp256k1Eth is ethereum-compatible address derivation from Secp256k1 public keys.
	Secp256k1Eth *secp256k1.PublicKey `json:"secp256k1eth,omitempty"`

	// Sr25519 address derivation.
	Sr25519 *sr25519.PublicKey `json:"sr25519,omitempty"`
}

// PublicKey returns the public key of the authentication/address derivation specification.
func (as *SignatureAddressSpec) PublicKey() PublicKey {
	switch {
	case as.Ed25519 != nil:
		return PublicKey{PublicKey: as.Ed25519}
	case as.Secp256k1Eth != nil:
		return PublicKey{PublicKey: as.Secp256k1Eth}
	case as.Sr25519 != nil:
		return PublicKey{PublicKey: as.Sr25519}
	}
	return PublicKey{}
}

// NewSignatureAddressSpecEd25519 creates a new address specification for an Ed25519 public key.
func NewSignatureAddressSpecEd25519(pk ed25519.PublicKey) SignatureAddressSpec {
	return SignatureAddressSpec{Ed25519: &pk}
}

// NewSignatureAddressSpecSecp256k1Eth creates a new Ethereum-compatible address specification for
// an Secp256k1 public key.
func NewSignatureAddressSpecSecp256k1Eth(pk secp256k1.PublicKey) SignatureAddressSpec {
	return SignatureAddressSpec{Secp256k1Eth: &pk}
}

// NewSignatureAddressSpecSr25519 creates a new address specification for an Sr25519 public key.
func NewSignatureAddressSpecSr25519(pk sr25519.PublicKey) SignatureAddressSpec {
	return SignatureAddressSpec{Sr25519: &pk}
}

// Address is the account address.
type Address address.Address

// MarshalBinary encodes an address into binary form.
func (a Address) MarshalBinary() ([]byte, error) {
	return (address.Address)(a).MarshalBinary()
}

// UnmarshalBinary decodes a binary marshaled address.
func (a *Address) UnmarshalBinary(data []byte) error {
	return (*address.Address)(a).UnmarshalBinary(data)
}

// MarshalText encodes an address into text form.
func (a Address) MarshalText() ([]byte, error) {
	return (address.Address)(a).MarshalBech32(AddressBech32HRP)
}

// UnmarshalText decodes a text marshaled address.
func (a *Address) UnmarshalText(text []byte) error {
	return (*address.Address)(a).UnmarshalBech32(AddressBech32HRP, text)
}

// Equal compares vs another address for equality.
func (a Address) Equal(cmp Address) bool {
	return (address.Address)(a).Equal((address.Address)(cmp))
}

// String returns the string representation of an address.
func (a Address) String() string {
	bech32Addr, err := bech32.Encode(AddressBech32HRP.String(), a[:])
	if err != nil {
		return "[malformed]"
	}
	return bech32Addr
}

// ConsensusAddress converts this address into a consensus-layer address type.
func (a Address) ConsensusAddress() staking.Address {
	return (staking.Address)((address.Address)(a))
}

// NewAddress creates a new address from the given signature address specification.
func NewAddress(spec SignatureAddressSpec) (a Address) {
	var (
		ctx    address.Context
		pkData []byte
	)
	switch {
	case spec.Ed25519 != nil:
		ctx = AddressV0Ed25519Context
		pkData, _ = spec.Ed25519.MarshalBinary()
	case spec.Secp256k1Eth != nil:
		ctx = AddressV0Secp256k1EthContext
		// Use a scheme such that we can compute Secp256k1 addresses from Ethereum
		// addresses as this makes things more interoperable.
		h := sha3.NewLegacyKeccak256()
		untaggedPk, _ := spec.Secp256k1Eth.MarshalBinaryUncompressedUntagged()
		h.Write(untaggedPk)
		pkData = h.Sum(nil)[32-20:]
	case spec.Sr25519 != nil:
		ctx = AddressV0Sr25519Context
		pkData, _ = spec.Sr25519.MarshalBinary()
	default:
		panic("address: unsupported public key type")
	}
	return (Address)(address.NewAddress(ctx, pkData))
}

// NewAddressRaw creates a new address from passed address context and data.
func NewAddressRaw(ctx address.Context, data []byte) Address {
	return (Address)(address.NewAddress(ctx, data))
}

// NewAddressForModule creates a new address for a specific module and raw kind.
func NewAddressForModule(module string, kind []byte) Address {
	moduleBytes := []byte(module)
	sepBytes := []byte(".")
	data := make([]byte, 0, len(moduleBytes)+len(kind)+1)
	data = append(data,
		moduleBytes...)
	data = append(data,
		sepBytes...)
	data = append(data,
		kind...,
	)
	return (Address)(address.NewAddress(AddressV0ModuleContext, data))
}

// NewAddressFromBech32 creates a new address from the given bech-32 encoded string.
//
// Panics in case of errors -- use UnmarshalText if you want to handle errors.
func NewAddressFromBech32(data string) (a Address) {
	err := a.UnmarshalText([]byte(data))
	if err != nil {
		panic(err)
	}
	return
}

// NewAddressFromMultisig creates a new address from the given multisig configuration.
func NewAddressFromMultisig(config *MultisigConfig) Address {
	return (Address)(address.NewAddress(AddressV0MultisigContext, cbor.Marshal(config)))
}

// NewAddressFromConsensus converts a consensus layer address into an address.
func NewAddressFromConsensus(addr staking.Address) Address {
	return (Address)((address.Address)(addr))
}
