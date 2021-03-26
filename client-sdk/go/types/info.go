package types

import (
	"github.com/oasisprotocol/oasis-core/go/common"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature"
)

// RuntimeInfo is information about a runtime.
type RuntimeInfo struct {
	// ID is the runtime identifier.
	ID common.Namespace
	// ChainContext is the chain domain separation context used by the runtime.
	ChainContext signature.Context
}
