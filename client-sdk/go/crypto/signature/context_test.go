package signature

import (
	"testing"

	"github.com/stretchr/testify/require"

	"github.com/oasisprotocol/oasis-core/go/common"
)

func TestDeriveChainContext(t *testing.T) {
	require := require.New(t)

	var runtimeID common.Namespace
	_ = runtimeID.UnmarshalHex("8000000000000000000000000000000000000000000000000000000000000000")

	chainCtx := DeriveChainContext(runtimeID, "643fb06848be7e970af3b5b2d772eb8cfb30499c8162bc18ac03df2f5e22520e")
	require.Equal("ca4842870b97a6d5c0d025adce0b6a0dec94d2ba192ede70f96349cfbe3628b9", string(chainCtx))

	ctx1 := chainCtx.New([]byte("oasis-runtime-sdk/tx: v0"))
	require.Equal("oasis-runtime-sdk/tx: v0 for chain ca4842870b97a6d5c0d025adce0b6a0dec94d2ba192ede70f96349cfbe3628b9", string(ctx1))
}
