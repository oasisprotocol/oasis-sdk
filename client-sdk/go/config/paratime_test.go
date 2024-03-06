package config

import (
	"testing"

	"github.com/stretchr/testify/require"
)

func TestValidateParaTime(t *testing.T) {
	require := require.New(t)

	p := ParaTime{
		Description: "Test ParaTime.",
		ID:          "000000000000000000000000000000000000000000000000f80306c9858e7279",
		Denominations: map[string]*DenominationInfo{
			NativeDenominationKey: {
				Symbol:   "FOO",
				Decimals: 18,
			},
			"BAR": {
				Symbol:   "BARfoo",
				Decimals: 9,
			},
			"foo": {
				Symbol:   "FOO",
				Decimals: 9,
			},
		},
	}
	err := p.Validate()
	require.NoError(err, "Validate should succeed with valid configuration")

	p.ConsensusDenomination = NativeDenominationKey
	err = p.Validate()
	require.NoError(err, "Validate should succeed with valid consensus denomination")
	p.ConsensusDenomination = "BAR"
	err = p.Validate()
	require.NoError(err, "Validate should succeed with valid consensus denomination")
	p.ConsensusDenomination = "FOO"
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
				Symbol:   "FOO",
				Decimals: 18,
			},
			"BAR": {
				Symbol:   "BARfoo",
				Decimals: 9,
			},
			"low": {
				Symbol:   "LOWfoo",
				Decimals: 9,
			},
		},
	}
	err := p.Validate()
	require.NoError(err, "Validate should succeed with valid configuration")

	di := p.GetDenominationInfo("")
	require.NotNil(di, "GetDenominationInfo should return a non-nil denomination info")
	require.Equal(di.Symbol, "FOO")
	require.EqualValues(di.Decimals, 18)

	di = p.GetDenominationInfo("BAR")
	require.NotNil(di, "GetDenominationInfo should return a non-nil denomination info")
	require.Equal(di.Symbol, "BARfoo")
	require.EqualValues(di.Decimals, 9)

	di = p.GetDenominationInfo("LOW")
	require.NotNil(di, "GetDenominationInfo should return a non-nil denomination info")
	require.Equal(di.Symbol, "LOWfoo")
	require.EqualValues(di.Decimals, 9)

	di = p.GetDenominationInfo("DEFAULT")
	require.NotNil(di, "GetDenominationInfo should return a non-nil denomination info")
	require.Equal(di.Symbol, "DEFAULT")
	require.EqualValues(di.Decimals, 9)
}
