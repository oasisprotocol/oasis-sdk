package core

import (
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

// EstimateGasQuery is the body of the core.EstimateGas query.
type EstimateGasQuery struct {
	// Caller is the address of the caller for which to do estimation. If not specified the
	// authentication information from the passed transaction is used.
	Caller *types.CallerAddress `json:"caller,omitempty"`
	// Tx is the unsigned transaction to estimate.
	Tx *types.Transaction `json:"tx"`
}

// ModuleName is the core module name.
const ModuleName = "core"

const (
	// GasUsedEventCode is the event code for the gas used event.
	GasUsedEventCode = 1
)

// GasUsedEvent is a gas used event.
type GasUsedEvent struct {
	Amount uint64 `json:"amount"`
}

// Event is a core module event.
type Event struct {
	GasUsed *GasUsedEvent
}
