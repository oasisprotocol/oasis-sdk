package types //nolint:revive

import (
	"context"
	"fmt"
	"io"

	"github.com/oasisprotocol/oasis-core/go/common/prettyprint"
	"github.com/oasisprotocol/oasis-core/go/common/quantity"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/config"
	sdkSignature "github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature"
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

// FormatNamedAddress returns a human-friendly representation of an address.
//
// It prints the name (if known) followed by the preferred form of the address
// in parentheses. If an Ethereum hex address mapping is provided for the native
// address, it is used; otherwise the native Bech32 address is used.
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

	// Preserve the user-provided Ethereum address even when the account is unnamed.
	if origToHex, ok := origToHexForAddress(ctx, addr); ok {
		native := addr.String()
		name := ""
		if names != nil {
			name = names[native]
		}

		switch {
		case name == "":
			return origToHex
		case name == origToHex:
			return origToHex
		default:
			return fmt.Sprintf("%s (%s)", name, origToHex)
		}
	}

	return FormatNamedAddressWith(names, ethMap, addr)
}

// FormatNamedAddressWith is a pure helper for address formatting to make testing easier.
func FormatNamedAddressWith(names AccountNames, ethMap map[string]string, addr Address) string {
	native := addr.String()

	name := ""
	if names != nil {
		name = names[native]
	}
	if name == "" {
		return native
	}

	preferred := native
	if ethMap != nil {
		if hex := ethMap[native]; hex != "" {
			preferred = hex
		}
	}

	// Guard against redundant "name (name)" output.
	if name == preferred {
		return preferred
	}

	return fmt.Sprintf("%s (%s)", name, preferred)
}

func origToHexForAddress(ctx context.Context, addr Address) (string, bool) {
	sc, ok := ctx.Value(sdkSignature.ContextKeySigContext).(*sdkSignature.RichContext)
	if !ok || sc == nil || sc.TxDetails == nil || sc.TxDetails.OrigTo == nil {
		return "", false
	}

	// Only apply OrigTo if it matches the address being printed.
	derived := NewAddressFromEth(sc.TxDetails.OrigTo.Bytes())
	if !derived.Equal(addr) {
		return "", false
	}

	return sc.TxDetails.OrigTo.Hex(), true
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
