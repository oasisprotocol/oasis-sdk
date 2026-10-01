package config

import (
	"testing"

	"github.com/stretchr/testify/require"
)

const (
	symbolFOO     = "FOO"
	symbolBAR     = "BAR"
	symbolBARfoo  = "BARfoo"
	symbolLOW     = "LOW"
	symbolLOWfoo  = "LOWfoo"
	symbolDEFAULT = "DEFAULT"
)

func TestValidateParaTime(t *testing.T) {
	require := require.New(t)

	p := ParaTime{
		Description: "Test ParaTime.",
		ID:          "000000000000000000000000000000000000000000000000f80306c9858e7279",
		Denominations: map[string]*DenominationInfo{
			NativeDenominationKey: {
				Symbol:   symbolFOO, //nolint:goconst
				Decimals: 18,
			},
			"BAR": {
				Symbol:   symbolBARfoo,
				Decimals: 9,
			},
			"foo": {
				Symbol:   symbolFOO,
				Decimals: 9,
			},
		},
	}
	err := p.Validate()
	require.NoError(err, "Validate should succeed with valid configuration")

	p.ConsensusDenomination = NativeDenominationKey
	err = p.Validate()
	require.NoError(err, "Validate should succeed with valid consensus denomination")
	p.ConsensusDenomination = symbolBAR
	err = p.Validate()
	require.NoError(err, "Validate should succeed with valid consensus denomination")
	p.ConsensusDenomination = symbolFOO
	err = p.Validate()
	require.NoError(err, "Validate should succeed with valid consensus denomination")

	invalid := p
	invalid.ID = "invalid"
	err = invalid.Validate()
	require.Error(err, "Validate should fail with invalid ID")

	invalid = p
	invalid.ConsensusDenomination = "invalid"
	err = invalid.Validate()
	require.Error(err, "Validate should fail with invalid consensus denomination")
}

func TestDenominationInfo(t *testing.T) {
	require := require.New(t)

	p := ParaTime{
		Description: "Test ParaTime.",
		ID:          "000000000000000000000000000000000000000000000000f80306c9858e7279",
		Denominations: map[string]*DenominationInfo{
			NativeDenominationKey: {
				Symbol:   symbolFOO,
				Decimals: 18,
			},
			"BAR": {
				Symbol:   symbolBARfoo,
				Decimals: 9,
			},
			"low": {
				Symbol:   symbolLOWfoo,
				Decimals: 9,
			},
		},
	}
	err := p.Validate()
	require.NoError(err, "Validate should succeed with valid configuration")

	di := p.GetDenominationInfo("")
	require.NotNil(di, "GetDenominationInfo should return a non-nil denomination info")
	require.Equal(di.Symbol, symbolFOO)
	require.EqualValues(di.Decimals, 18)

	di = p.GetDenominationInfo(symbolBAR)
	require.NotNil(di, "GetDenominationInfo should return a non-nil denomination info")
	require.Equal(di.Symbol, symbolBARfoo)
	require.EqualValues(di.Decimals, 9)

	di = p.GetDenominationInfo(symbolLOW)
	require.NotNil(di, "GetDenominationInfo should return a non-nil denomination info")
	require.Equal(di.Symbol, symbolLOWfoo)
	require.EqualValues(di.Decimals, 9)

	di = p.GetDenominationInfo(symbolDEFAULT)
	require.NotNil(di, "GetDenominationInfo should return a non-nil denomination info")
	require.Equal(di.Symbol, symbolDEFAULT)
	require.EqualValues(di.Decimals, 9)
}
