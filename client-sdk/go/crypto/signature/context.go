package signature

import (
	"github.com/oasisprotocol/oasis-core/go/common"
	"github.com/oasisprotocol/oasis-core/go/common/cbor"
	"github.com/oasisprotocol/oasis-core/go/common/crypto/hash"

	ethCommon "github.com/ethereum/go-ethereum/common"
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

type HwSignRtMetadata struct {
	RuntimeID    common.Namespace   `json:"runtime_id"`
	ChainContext string             `json:"chain_context"`
	OrigTo       *ethCommon.Address `json:"orig_to,omitempty"`
}

// EncodeAsMetadata encodes runtime ID, consensus chain context and tx-specific details
// as defined in ADR 14.
func EncodeAsMetadata(runtimeID common.Namespace, consensusChainContext string, txDetails map[string]interface{}) []byte {
	// Optional transaction details.
	origToEthAddr := txDetails["origTo"].(*ethCommon.Address)
	metadata := HwSignRtMetadata{
		RuntimeID:    runtimeID,
		ChainContext: consensusChainContext,
		OrigTo:       origToEthAddr,
	}

	return cbor.Marshal(metadata)
}
