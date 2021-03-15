package types

import (
	"encoding/hex"
	"testing"

	"github.com/stretchr/testify/require"

	"github.com/oasisprotocol/oasis-core/go/common/cbor"
	"github.com/oasisprotocol/oasis-core/go/common/quantity"
)

func TestToken(t *testing.T) {
	require := require.New(t)

	for _, tc := range []struct {
		value       uint64
		denom       Denomination
		expectedHex string
	}{
		// Native denomination.
		{0, NativeDenomination, "824040"},
		{1, NativeDenomination, "82410140"},
		{1000, NativeDenomination, "824203e840"},
		// Custom denomination.
		{0, Denomination("test"), "82404474657374"},
		{1, Denomination("test"), "8241014474657374"},
		{1000, Denomination("test"), "824203e84474657374"},
	} {
		token := NewBaseUnits(*quantity.NewFromUint64(tc.value), tc.denom)
		enc := cbor.Marshal(token)

		require.EqualValues(tc.expectedHex, hex.EncodeToString(enc), "serialization should match")

		var dec BaseUnits
		err := cbor.Unmarshal(enc, &dec)
		require.NoError(err, "deserialization should succeed")
		require.EqualValues(token, dec, "serialization should round-trip")
	}
}
