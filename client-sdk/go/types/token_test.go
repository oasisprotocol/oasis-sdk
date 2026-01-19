package types //nolint:revive

import (
	"bytes"
	"context"
	"encoding/hex"
	"testing"

	"github.com/stretchr/testify/require"

	"github.com/oasisprotocol/oasis-core/go/common/cbor"
	"github.com/oasisprotocol/oasis-core/go/common/quantity"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/config"
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

func TestPrettyPrintToAmount(t *testing.T) {
	require := require.New(t)

	ptCfg := &config.ParaTime{
		Denominations: map[string]*config.DenominationInfo{
			"_": {
				Symbol:   "TEST",
				Decimals: 18,
			},
		},
	}

	ctx := context.Background()
	ctx = context.WithValue(ctx, config.ContextKeyParaTimeCfg, ptCfg)
	ctx = context.WithValue(ctx, ContextKeyAccountNames, AccountNames{
		"oasis1qrk58a6j2qn065m6p06jgjyt032f7qucy5wqeqpt": "test:dave",
	})

	to := Address{}
	err := to.UnmarshalText([]byte("oasis1qrk58a6j2qn065m6p06jgjyt032f7qucy5wqeqpt"))
	require.NoError(err)
	qt := Quantity{}
	err = qt.UnmarshalText([]byte("50000000000000000000"))
	require.NoError(err)
	amt := BaseUnits{
		Amount:       qt,
		Denomination: NativeDenomination,
	}

	var buf bytes.Buffer
	PrettyPrintToAmount(ctx, "", &buf, &to, amt)
	require.Equal("To: test:dave (oasis1qrk58a6j2qn065m6p06jgjyt032f7qucy5wqeqpt)\nAmount: 50.0 TEST\n", buf.String())

	// Unnamed address with eth mapping should display eth hex with native in parentheses.
	buf.Reset()
	ethHex := "0x60a6321eA71d37102Dbf923AAe2E08d005C4e403"
	ethBytes, err2 := hex.DecodeString(ethHex[2:])
	require.NoError(err2)
	ethTo := NewAddressFromEth(ethBytes)
	ctx2 := context.Background()
	ctx2 = context.WithValue(ctx2, config.ContextKeyParaTimeCfg, ptCfg)
	ctx2 = context.WithValue(ctx2, ContextKeyAccountEthMap, map[string]string{
		ethTo.String(): ethHex,
	})
	PrettyPrintToAmount(ctx2, "", &buf, &ethTo, amt)
	require.Equal("To: "+ethHex+" ("+ethTo.String()+")\nAmount: 50.0 TEST\n", buf.String())

	// No ParaTime set. Amount cannot be correctly determined.
	buf.Reset()
	ctx = context.WithValue(ctx, config.ContextKeyParaTimeCfg, nil)
	PrettyPrintToAmount(ctx, "", &buf, &to, amt)
	require.Equal("To: test:dave (oasis1qrk58a6j2qn065m6p06jgjyt032f7qucy5wqeqpt)\nAmount: <error: ParaTime information not available>\n", buf.String())
}

func TestFormatNamedAddressWith(t *testing.T) {
	require := require.New(t)

	ethHex := "0x60a6321ea71d37102dbf923aae2e08d005c4e403"
	ethBytes, err := hex.DecodeString(ethHex[2:])
	require.NoError(err)

	addr := NewAddressFromEth(ethBytes)
	native := addr.String()

	t.Run("unknown no eth returns native", func(_ *testing.T) {
		require.Equal(native, FormatNamedAddressWith(nil, nil, addr))
		require.Equal(native, FormatNamedAddressWith(AccountNames{}, map[string]string{}, addr))
	})

	t.Run("unknown with eth returns eth hex", func(_ *testing.T) {
		ethMap := map[string]string{native: ethHex}
		require.Equal(ethHex+" ("+native+")", FormatNamedAddressWith(nil, ethMap, addr))
		require.Equal(ethHex+" ("+native+")", FormatNamedAddressWith(AccountNames{}, ethMap, addr))
	})

	t.Run("native fallback when eth unknown", func(_ *testing.T) {
		names := AccountNames{native: "my"}
		require.Equal("my ("+native+")", FormatNamedAddressWith(names, nil, addr))
		require.Equal("my ("+native+")", FormatNamedAddressWith(names, map[string]string{}, addr))
		require.Equal("my ("+native+")", FormatNamedAddressWith(names, map[string]string{native: ""}, addr))
	})

	t.Run("eth preferred when known", func(_ *testing.T) {
		names := AccountNames{native: "my"}
		ethMap := map[string]string{native: ethHex}
		require.Equal("my ("+ethHex+")", FormatNamedAddressWith(names, ethMap, addr))
	})

	t.Run("name equals preferred yields preferred", func(_ *testing.T) {
		names := AccountNames{native: native}
		require.Equal(native, FormatNamedAddressWith(names, nil, addr))
	})

	t.Run("ctx wrapper reads maps", func(_ *testing.T) {
		ctx := context.Background()
		ctx = context.WithValue(ctx, ContextKeyAccountNames, AccountNames{native: "my"})
		ctx = context.WithValue(ctx, ContextKeyAccountEthMap, map[string]string{native: ethHex})
		require.Equal("my ("+ethHex+")", FormatNamedAddress(ctx, addr))
	})
}
