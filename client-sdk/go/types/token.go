package types

import (
	"fmt"

	"github.com/oasisprotocol/oasis-core/go/common/quantity"
)

// Quantity is a arbitrary precision unsigned integer that never underflows.
type Quantity = quantity.Quantity

// NativeDenomination is the denomination in native token.
var NativeDenomination = Denomination([]byte{})

// MaxDenominationSize is the maximum length of a denomination.
const MaxDenominationSize = 32

// Denomination is the name/type of the token.
type Denomination string

// MarshalBinary encodes a denomination into binary form.
func (d Denomination) MarshalBinary() ([]byte, error) {
	return []byte(d), nil
}

// UnmarshalBinary decodes a binary marshaled denomination.
func (a *Denomination) UnmarshalBinary(data []byte) error {
	if len(data) > MaxDenominationSize {
		return fmt.Errorf("malformed denomination")
	}
	*a = Denomination(string(data))
	return nil
}

// String returns a string representation of this denomination.
func (d Denomination) String() string {
	if d.IsNative() {
		return "<native>"
	}
	return string(d)
}

// IsNative checks whether the denomination represents the native token.
func (d Denomination) IsNative() bool {
	return len(d) == 0
}

// BaseUnits is the token amount of given denomination in base units.
type BaseUnits struct {
	_ struct{} `cbor:",toarray"` // nolint

	Amount       quantity.Quantity
	Denomination Denomination
}

// String returns a string representation of this token amount.
func (bu BaseUnits) String() string {
	return fmt.Sprintf("%s %s", bu.Amount.String(), bu.Denomination.String())
}

// NewBaseUnits creates a new token amount of given denomination.
func NewBaseUnits(amount quantity.Quantity, denomination Denomination) BaseUnits {
	return BaseUnits{
		Amount:       amount,
		Denomination: denomination,
	}
}
