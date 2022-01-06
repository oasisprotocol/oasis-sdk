package helpers

import (
	"fmt"

	"github.com/shopspring/decimal"

	"github.com/oasisprotocol/oasis-core/go/common/prettyprint"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/config"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

// ParseConsensusDenomination parses an amount for the consensus layer denomination.
func ParseConsensusDenomination(net *config.Network, amount string) (*types.Quantity, error) {
	bu, err := parseDenomination(&net.Denomination, amount, types.NativeDenomination)
	if err != nil {
		return nil, err
	}
	return &bu.Amount, nil
}

// ParseParaTimeDenomination parses an amount for the given ParaTime denomination.
func ParseParaTimeDenomination(pt *config.ParaTime, amount string, denom types.Denomination) (*types.BaseUnits, error) {
	return parseDenomination(pt.GetDenominationInfo(denom), amount, denom)
}

func parseDenomination(di *config.DenominationInfo, amount string, denom types.Denomination) (*types.BaseUnits, error) {
	v, err := decimal.NewFromString(amount)
	if err != nil {
		return nil, err
	}

	// Multiply to get the number of base units.
	var q types.Quantity
	baseUnits := v.Mul(decimal.New(1, int32(di.Decimals)))
	if err := q.FromBigInt(baseUnits.BigInt()); err != nil {
		return nil, err
	}
	bu := types.NewBaseUnits(q, denom)
	return &bu, nil
}

// FormatConsensusDenomination formats the given base unit amount as a consensus layer denomination.
func FormatConsensusDenomination(net *config.Network, amount types.Quantity) string {
	return formatDenomination(&net.Denomination, amount)
}

// FormatParaTimeDenomination formats the given base unit amount as a ParaTime denomination.
func FormatParaTimeDenomination(pt *config.ParaTime, amount types.BaseUnits) string {
	return formatDenomination(pt.GetDenominationInfo(amount.Denomination), amount.Amount)
}

func formatDenomination(di *config.DenominationInfo, amount types.Quantity) string {
	return fmt.Sprintf("%s %s", prettyprint.QuantityFrac(amount, di.Decimals), di.Symbol)
}
