package rofl

import (
	"encoding"
	"encoding/binary"

	"github.com/oasisprotocol/oasis-core/go/common/crypto/address"
	"github.com/oasisprotocol/oasis-core/go/common/encoding/bech32"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

var (
	// AppIDV0CRIContext is the unique context for v0 creator/round/index application identifiers.
	AppIDV0CRIContext = address.NewContext("oasis-sdk/rofl: cri app id", 0)
	// AppIDV0CNContext is the unique context for v0 creator/nonce application identifiers.
	AppIDV0CNContext = address.NewContext("oasis-sdk/rofl: cn app id", 0)
	// AppIDV0GlobalNameContext is the unique context for v0 global name application identifiers.
	AppIDV0GlobalNameContext = address.NewContext("oasis-sdk/rofl: global name app id", 0)
	// AppIDBech32HRP is the unique human readable part of Bech32 encoded application identifiers.
	AppIDBech32HRP = address.NewBech32HRP("rofl")

	_ encoding.BinaryMarshaler   = AppID{}
	_ encoding.BinaryUnmarshaler = (*AppID)(nil)
	_ encoding.TextMarshaler     = AppID{}
	_ encoding.TextUnmarshaler   = (*AppID)(nil)
)

// AppID is the ROFL application identifier.
type AppID address.Address

// MarshalBinary encodes an application identifier into binary form.
func (a AppID) MarshalBinary() ([]byte, error) {
	return (address.Address)(a).MarshalBinary()
}

// UnmarshalBinary decodes a binary marshaled application identifier.
func (a *AppID) UnmarshalBinary(data []byte) error {
	return (*address.Address)(a).UnmarshalBinary(data)
}

// MarshalText encodes an application identifier into text form.
func (a AppID) MarshalText() ([]byte, error) {
	return (address.Address)(a).MarshalBech32(AppIDBech32HRP)
}

// UnmarshalText decodes a text marshaled application identifier.
func (a *AppID) UnmarshalText(text []byte) error {
	return (*address.Address)(a).UnmarshalBech32(AppIDBech32HRP, text)
}

// Equal compares vs another application identifier for equality.
func (a AppID) Equal(cmp AppID) bool {
	return (address.Address)(a).Equal((address.Address)(cmp))
}

// String returns the string representation of an application identifier.
func (a AppID) String() string {
	bech32Addr, err := bech32.Encode(AppIDBech32HRP.String(), a[:])
	if err != nil {
		return "[malformed]"
	}
	return bech32Addr
}

// NewAppIDCreatorRoundIndex creates a new application identifier from the given creator/round/index
// tuple.
func NewAppIDCreatorRoundIndex(creator types.Address, round uint64, index uint32) AppID {
	data := make([]byte, address.Size+8+4)

	rawCreator, _ := creator.MarshalBinary()
	copy(data[:address.Size], rawCreator)

	binary.BigEndian.PutUint64(data[address.Size:], round)
	binary.BigEndian.PutUint32(data[address.Size+8:], index)

	return NewAppIDRaw(AppIDV0CRIContext, data)
}

// NewAppIDCreatorNonce creates a new application identifier from the given creator/nonce tuple.
func NewAppIDCreatorNonce(creator types.Address, nonce uint64) AppID {
	data := make([]byte, address.Size+8)

	rawCreator, _ := creator.MarshalBinary()
	copy(data[:address.Size], rawCreator)

	binary.BigEndian.PutUint64(data[address.Size:], nonce)

	return NewAppIDRaw(AppIDV0CNContext, data)
}

// NewAppIDGlobalName creates a new application identifier from the given global name.
func NewAppIDGlobalName(name string) AppID {
	return NewAppIDRaw(AppIDV0GlobalNameContext, []byte(name))
}

// NewAppIDRaw creates a new application identifier from passed context and data.
func NewAppIDRaw(ctx address.Context, data []byte) AppID {
	return (AppID)(address.NewAddress(ctx, data))
}

// NewAppIDFromBech32 creates a new application identifier from the given bech-32 encoded string.
//
// Panics in case of errors -- use UnmarshalText if you want to handle errors.
func NewAppIDFromBech32(data string) (a AppID) {
	err := a.UnmarshalText([]byte(data))
	if err != nil {
		panic(err)
	}
	return
}
