package types //nolint:revive

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
		_, _ = fmt.Fprintf(w, "<error: ParaTime information not available>")
		return
	}
	di := pt.GetDenominationInfo(string(bu.Denomination))
	_, _ = fmt.Fprintf(w, "%s %s", prettyprint.QuantityFrac(bu.Amount, di.Decimals), di.Symbol)
}

// PrettyType returns a representation of the type that can be used for pretty printing.
func (bu *BaseUnits) PrettyType() (any, error) {
	return bu, nil
}

// NewBaseUnits creates a new token amount of given denomination.
func NewBaseUnits(amount quantity.Quantity, denomination Denomination) BaseUnits {
	return BaseUnits{
		Amount:       amount,
		Denomination: denomination,
	}
}

// FormatNamedAddress is like FormatNamedAddressWith but reads the name and eth maps from ctx.
func FormatNamedAddress(ctx context.Context, addr Address) string {
	var (
		names  AccountNames
		ethMap map[string]string
	)
	if v, ok := ctx.Value(ContextKeyAccountNames).(AccountNames); ok {
		names = v
	}
	if v, ok := ctx.Value(ContextKeyAccountEthMap).(map[string]string); ok {
		ethMap = v
	}

	return FormatNamedAddressWith(names, ethMap, addr)
}

// FormatNamedAddressWith formats an address for display. Output cases:
//   - Named + eth known:    "name (0x...)"
//   - Named + eth unknown:  "name (oasis1...)"
//   - Unnamed + eth known:  "0x... (oasis1...)"
//   - Unnamed + eth unknown: "oasis1..."
func FormatNamedAddressWith(names AccountNames, ethMap map[string]string, addr Address) string {
	native := addr.String()

	ethHex := ""
	if ethMap != nil {
		if hex := ethMap[native]; hex != "" {
			ethHex = hex
		}
	}

	name := ""
	if names != nil {
		name = names[native]
	}

	// Named address.
	if name != "" {
		preferred := native
		if ethHex != "" {
			preferred = ethHex
		}
		if name == preferred {
			return preferred
		}
		return fmt.Sprintf("%s (%s)", name, preferred)
	}

	// Unnamed address.
	if ethHex != "" {
		return fmt.Sprintf("%s (%s)", ethHex, native)
	}

	return native
}

// PrettyPrintToAmount is a helper for printing To-Amount transaction bodies (e.g. transfer, deposit, withdraw).
func PrettyPrintToAmount(ctx context.Context, prefix string, w io.Writer, to *Address, amount BaseUnits) {
	toStr := "Self"
	if to != nil {
		toStr = FormatNamedAddress(ctx, *to)
	}
	_, _ = fmt.Fprintf(w, "%sTo: %s\n", prefix, toStr)
	_, _ = fmt.Fprintf(w, "%sAmount: ", prefix)
	amount.PrettyPrint(ctx, prefix, w)
	_, _ = fmt.Fprintln(w)
}
