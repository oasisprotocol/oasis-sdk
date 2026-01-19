package consensusaccounts

import (
	"bytes"
	"context"
	"encoding/hex"
	"testing"

	"github.com/stretchr/testify/require"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"

	"github.com/oasisprotocol/oasis-core/go/common/quantity"
)

func TestUndelegatePrettyPrintFromNamedAddress(t *testing.T) {
	require := require.New(t)

	ethHex := "0x60a6321ea71d37102dbf923aae2e08d005c4e403"
	ethBytes, err := hex.DecodeString(ethHex[2:])
	require.NoError(err)

	addr := types.NewAddressFromEth(ethBytes)
	native := addr.String()

	shares := *quantity.NewFromUint64(1234)

	ctx := context.Background()
	ctx = context.WithValue(ctx, types.ContextKeyAccountNames, types.AccountNames{native: "my"})
	ctx = context.WithValue(ctx, types.ContextKeyAccountEthMap, map[string]string{native: ethHex})

	ud := Undelegate{
		From:   addr,
		Shares: shares,
	}

	var buf bytes.Buffer
	ud.PrettyPrint(ctx, "", &buf)

	require.Equal("From: my ("+ethHex+")\nShares: "+shares.String()+"\n", buf.String())
}
