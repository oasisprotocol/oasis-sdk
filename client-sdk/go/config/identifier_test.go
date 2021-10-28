package config

import (
	"testing"

	"github.com/stretchr/testify/require"
)

func TestValidateIdentifier(t *testing.T) {
	require := require.New(t)

	for _, tc := range []struct {
		id    string
		valid bool
	}{
		{"", false},
		{"ABC", false},
		{"abcabcabcabcabcabcabcabcabcabcabcabcabcabcabcabcabcabcabcabcabcabc", false},
		{"abc(", false},
		{"222", true},
		{"abc", true},
		{"abc_2", true},
	} {
		if tc.valid {
			require.NoError(ValidateIdentifier(tc.id), tc.id)
		} else {
			require.Error(ValidateIdentifier(tc.id), tc.id)
		}
	}
}
