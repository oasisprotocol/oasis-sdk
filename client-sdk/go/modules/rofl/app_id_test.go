package rofl

import (
	"testing"

	"github.com/stretchr/testify/require"

	sdkTesting "github.com/oasisprotocol/oasis-sdk/client-sdk/go/testing"
)

func TestIdentifierV0(t *testing.T) {
	require := require.New(t)

	appID := NewAppIDCreatorRoundIndex(sdkTesting.Alice.Address, 42, 0)
	require.Equal("rofl1qr98wz5t6q4x8ng6a5l5v7rqlx90j3kcnun5dwht", appID.String())

	appID = NewAppIDCreatorRoundIndex(sdkTesting.Bob.Address, 42, 0)
	require.Equal("rofl1qrd45eaj4tf6l7mjw5prcukz75wdmwg6kggt6pnp", appID.String())

	appID = NewAppIDCreatorRoundIndex(sdkTesting.Bob.Address, 1, 0)
	require.Equal("rofl1qzmuyfwygnmfralgtwrqx8kcm587kwex9y8hf9hf", appID.String())

	appID = NewAppIDCreatorRoundIndex(sdkTesting.Bob.Address, 42, 1)
	require.Equal("rofl1qzmh56f52yd0tcqh757fahzc7ec49s8kaguyylvu", appID.String())

	appID = NewAppIDGlobalName("test global app")
	require.Equal("rofl1qrev5wq76npkmcv5wxkdxxcu4dhmu704yyl30h43", appID.String())
}
