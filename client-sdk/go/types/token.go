package types

import (
	"context"
	"fmt"
	"io"

	"github.com/oasisprotocol/oasis-core/go/common/prettyprint"
	"github.com/oasisprotocol/oasis-core/go/common/quantity"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/config"
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
func (d *Denomination) UnmarshalBinary(data []byte) error {
	if len(data) > MaxDenominationSize {
		return fmt.Errorf("malformed denomination")
	}
	*d = Denomination(string(data))
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
	_ struct{} `cbor:",toarray"`

	Amount       quantity.Quantity
	Denomination Denomination
}

// String returns a string representation of this token amount.
func (bu BaseUnits) String() string {
	return fmt.Sprintf("%s %s", bu.Amount.String(), bu.Denomination.String())
}

// PrettyPrint writes a pretty-printed representation of the base units to the given writer.
func (bu *BaseUnits) PrettyPrint(ctx context.Context, _ string, w io.Writer) {
	pt, ok := ctx.Value(config.ContextKeyParaTimeCfg).(*config.ParaTime)
	if !ok {
		fmt.Fprintf(w, "<error: ParaTime information not available>")
		return
	}
	di := pt.GetDenominationInfo(string(bu.Denomination))
	fmt.Fprintf(w, "%s %s", prettyprint.QuantityFrac(bu.Amount, di.Decimals), di.Symbol)
}

// PrettyType returns a representation of the type that can be used for pretty printing.
func (bu *BaseUnits) PrettyType() (interface{}, error) {
	return bu, nil
}

// NewBaseUnits creates a new token amount of given denomination.
func NewBaseUnits(amount quantity.Quantity, denomination Denomination) BaseUnits {
	return BaseUnits{
		Amount:       amount,
		Denomination: denomination,
	}
}

// FormatNamedAddress returns a human-friendly representation of an address:
// it prints the name (if known) followed by the preferred form of the address
// in parentheses. If an Ethereum hex address mapping is provided for the native
// address, it is used; otherwise the native Bech32 address is used.
func FormatNamedAddress(ctx context.Context, addr Address) string {
	name := ""
	if an, ok := ctx.Value(ContextKeyAccountNames).(AccountNames); ok && an != nil {
		if n, exists := an[addr.String()]; exists {
			name = n
		}
	}
	if name == "" {
		// Unknown address; return native Bech32 form.
		return addr.String()
	}

	// Prefer Ethereum hex if available.
	if ethMap, ok := ctx.Value(ContextKeyAccountEthMap).(map[string]string); ok && ethMap != nil {
		if ethHex, has := ethMap[addr.String()]; has && ethHex != "" {
			return fmt.Sprintf("%s (%s)", name, ethHex)
		}
	}
	return fmt.Sprintf("%s (%s)", name, addr.String())
}

// PrettyPrintToAmount is a helper for printing To-Amount transaction bodies (e.g. transfer, deposit, withdraw).
func PrettyPrintToAmount(ctx context.Context, prefix string, w io.Writer, to *Address, amount BaseUnits) {
	toStr := "Self"
	if to != nil {
		toStr = FormatNamedAddress(ctx, *to)
	}

	fmt.Fprintf(w, "%sTo: %s\n", prefix, toStr)
	fmt.Fprintf(w, "%sAmount: ", prefix)
	amount.PrettyPrint(ctx, prefix, w)
	fmt.Fprintln(w)
}
