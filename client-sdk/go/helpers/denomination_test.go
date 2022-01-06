package helpers

import (
	"testing"

	"github.com/stretchr/testify/require"

	"github.com/oasisprotocol/oasis-core/go/common/quantity"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/config"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

func TestParseConsensusDenomination(t *testing.T) {
	require := require.New(t)

	net := config.Network{
		Denomination: config.DenominationInfo{
			Symbol:   "TEST",
			Decimals: 9,
		},
	}

	for _, tc := range []struct {
		amount   string
		valid    bool
		expected uint64
	}{
		{"", false, 0},
		{"0", true, 0},
		{"0.0", true, 0},
		{"0.0.0", false, 0},
		{"0.1", true, 100_000_000},
		{"1", true, 1_000_000_000},
		{"10", true, 10_000_000_000},
		{"10.123", true, 10_123_000_000},
		{"10.999999999", true, 10_999_999_999},
		{"10.9999999991", true, 10_999_999_999},
		{"10.9999999999", true, 10_999_999_999},
		{"10.999999999123456", true, 10_999_999_999},
	} {
		amount, err := ParseConsensusDenomination(&net, tc.amount)
		if tc.valid {
			require.NoError(err, tc.amount)
			require.EqualValues(quantity.NewFromUint64(tc.expected), amount, tc.amount)
		} else {
			require.Error(err, tc.amount)
		}
	}
}

func TestFormatConsensusDenomination(t *testing.T) {
	require := require.New(t)

	net := config.Network{
		Denomination: config.DenominationInfo{
			Symbol:   "TEST",
			Decimals: 9,
		},
	}

	for _, tc := range []struct {
		amount   uint64
		expected string
	}{
		{0, "0.0 TEST"},
		{1, "0.000000001 TEST"},
		{1_000_000, "0.001 TEST"},
		{1_000_000_000, "1.0 TEST"},
		{10_000_000_000, "10.0 TEST"},
		{10_123_000_000, "10.123 TEST"},
		{10_123_456_789, "10.123456789 TEST"},
	} {
		require.EqualValues(tc.expected, FormatConsensusDenomination(&net, *quantity.NewFromUint64(tc.amount)), "%d", tc.amount)
	}
}

func TestParseParaTimeDenomination(t *testing.T) {
	require := require.New(t)

	pt := config.ParaTime{
		ID: "0000000000000000000000000000000000000000000000000000000000000000",
		Denominations: map[string]*config.DenominationInfo{
			"_": {
				Symbol:   "TEST",
				Decimals: 5,
			},
			"X": {
				Symbol:   "OMG",
				Decimals: 3,
			},
		},
	}

	for _, tc := range []struct {
		amount   string
		denom    types.Denomination
		valid    bool
		expected uint64
	}{
		{"", types.NativeDenomination, false, 0},
		{"0", types.NativeDenomination, true, 0},
		{"0.0", types.NativeDenomination, true, 0},
		{"0.0.0", types.NativeDenomination, false, 0},
		{"0.1", types.NativeDenomination, true, 10_000},
		{"1", types.NativeDenomination, true, 100_000},
		{"10", types.NativeDenomination, true, 1_000_000},
		{"10.123", types.NativeDenomination, true, 1_0123_00},
		{"10.999999999", types.NativeDenomination, true, 1_099_999},
		{"10.9999999991", types.NativeDenomination, true, 1_099_999},
		{"10.9999999999", types.NativeDenomination, true, 1_099_999},
		{"10.999999999123456", types.NativeDenomination, true, 1_099_999},
		{"", types.Denomination("X"), false, 0},
		{"0", types.Denomination("X"), true, 0},
		{"0.0", types.Denomination("X"), true, 0},
		{"0.0.0", types.Denomination("X"), false, 0},
		{"0.1", types.Denomination("X"), true, 100},
		{"1", types.Denomination("X"), true, 1_000},
		{"10", types.Denomination("X"), true, 10_000},
		{"10.123", types.Denomination("X"), true, 10_123},
		{"10.999999999", types.Denomination("X"), true, 10_999},
		{"10.9999999991", types.Denomination("X"), true, 10_999},
		{"10.9999999999", types.Denomination("X"), true, 10_999},
		{"10.999999999123456", types.Denomination("X"), true, 10_999},
	} {
		amount, err := ParseParaTimeDenomination(&pt, tc.amount, tc.denom)
		if tc.valid {
			require.NoError(err, tc.amount)

			expected := types.NewBaseUnits(*quantity.NewFromUint64(tc.expected), tc.denom)
			require.EqualValues(&expected, amount, tc.amount)
		} else {
			require.Error(err, tc.amount)
		}
	}
}

func TestFormatParaTimeDenomination(t *testing.T) {
	require := require.New(t)

	pt := config.ParaTime{
		ID: "0000000000000000000000000000000000000000000000000000000000000000",
		Denominations: map[string]*config.DenominationInfo{
			"_": {
				Symbol:   "TEST",
				Decimals: 5,
			},
			"X": {
				Symbol:   "OMG",
				Decimals: 3,
			},
		},
	}

	for _, tc := range []struct {
		amount   uint64
		denom    types.Denomination
		expected string
	}{
		{0, types.NativeDenomination, "0.0 TEST"},
		{1, types.NativeDenomination, "0.00001 TEST"},
		{1_000_000, types.NativeDenomination, "10.0 TEST"},
		{1_000_000_000, types.NativeDenomination, "10000.0 TEST"},
		{10_000_000_000, types.NativeDenomination, "100000.0 TEST"},
		{10_123_000_000, types.NativeDenomination, "101230.0 TEST"},
		{10_123_456_789, types.NativeDenomination, "101234.56789 TEST"},
		{0, types.Denomination("X"), "0.0 OMG"},
		{1, types.Denomination("X"), "0.001 OMG"},
		{1_000_000, types.Denomination("X"), "1000.0 OMG"},
		{1_000_000_000, types.Denomination("X"), "1000000.0 OMG"},
		{10_000_000_000, types.Denomination("X"), "10000000.0 OMG"},
		{10_123_000_000, types.Denomination("X"), "10123000.0 OMG"},
		{10_123_456_789, types.Denomination("X"), "10123456.789 OMG"},
	} {
		amount := types.NewBaseUnits(*quantity.NewFromUint64(tc.amount), tc.denom)
		require.EqualValues(tc.expected, FormatParaTimeDenomination(&pt, amount), "%d", tc.amount)
	}
}
