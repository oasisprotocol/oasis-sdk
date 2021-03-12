package signature

import (
	"github.com/oasisprotocol/oasis-core/go/common"
	"github.com/oasisprotocol/oasis-core/go/common/crypto/hash"
)

const chainContextSeparator = " for chain "

// Context is the chain domain separation context.
type Context string

func (c Context) New(base []byte) []byte {
	ctx := append([]byte{}, base...)
	ctx = append(ctx, []byte(chainContextSeparator)...)
	ctx = append(ctx, []byte(c)...)
	return ctx
}

// DeriveChainContext derives the chain domain separation context for a given runtime.
func DeriveChainContext(runtimeID common.Namespace, consensusChainContext string) Context {
	rawRuntimeID, _ := runtimeID.MarshalBinary()
	return Context(hash.NewFromBytes(
		rawRuntimeID,
		[]byte(consensusChainContext),
	).String())
}
