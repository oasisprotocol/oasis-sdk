package signature

import (
	"encoding/hex"

	"github.com/oasisprotocol/oasis-core/go/common"
	"github.com/oasisprotocol/oasis-core/go/common/crypto/hash"

	ethCommon "github.com/ethereum/go-ethereum/common"
)

type contextKey string

const (
	chainContextSeparator = " for chain "

	// ContextKeySigContext is the key to retrieve the transaction's signature context object from a
	// context.
	ContextKeySigContext = contextKey("runtime/signature-context")
)

// Context is the interface used to derive the chain domain separation context.
type Context interface {
	// Derive derives the chain domain separation context used for signing transactions.
	Derive() []byte
}

// RawContext is chain domain separation which can be directly used for signing.
type RawContext []byte

// Derive derives the chain domain separation context used for signing transactions.
func (rc RawContext) Derive() []byte {
	return rc
}

// RichContext stores runtime ID, consensus chain context and optional
// transaction-specific details.
type RichContext struct {
	// RuntimeID used for deriving the signature context.
	RuntimeID common.Namespace

	// ChainContext used for deriving the signature context.
	ChainContext string

	// Base contains chain context separator base.
	Base []byte

	// TxDetails contains optional transaction-specific details.
	TxDetails *TxDetails
}

// Derive derives the chain domain separation context used for signing transactions.
func (sc *RichContext) Derive() []byte {
	ctx := append([]byte{}, sc.Base...)
	ctx = append(ctx, []byte(chainContextSeparator)...)

	c := hash.NewFromBytes(
		sc.RuntimeID[:],
		[]byte(sc.ChainContext),
	).String()
	ctx = append(ctx, []byte(c)...)

	return ctx
}

// TxDetails contains transaction-specific details.
type TxDetails struct {
	OrigTo *ethCommon.Address
}

// HwContext is ADR 14-compatible struct appropriate for CBOR encoding as Meta component.
type HwContext struct {
	RuntimeID    string `json:"runtime_id"`
	ChainContext string `json:"chain_context"`
	OrigTo       string `json:"orig_to,omitempty"`
}

// NewHwContext creates a new hardware-wallet context from in-memory Context object.
func NewHwContext(sc *RichContext) *HwContext {
	origTo := ""
	if sc.TxDetails != nil && sc.TxDetails.OrigTo != nil {
		origTo = hex.EncodeToString(sc.TxDetails.OrigTo.Bytes())
	}
	return &HwContext{
		RuntimeID:    sc.RuntimeID.Hex(),
		ChainContext: sc.ChainContext,
		OrigTo:       origTo,
	}
}
