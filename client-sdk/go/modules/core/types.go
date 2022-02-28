package core

import (
	"github.com/oasisprotocol/oasis-core/go/common/quantity"

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

// GasCosts are the consensus accounts module gas costs.
type GasCosts struct {
	TxByte                   uint64 `json:"tx_byte"`
	AuthSignature            uint64 `json:"auth_signature"`
	AuthMultisigSigner       uint64 `json:"auth_multisig_signer"`
	CallformatX25519Deoxysii uint64 `json:"callformat_x25519_deoxysii"`
}

// Parameters are the parameters for the consensus accounts module.
type Parameters struct {
	MaxBatchGas        uint64                                   `json:"max_batch_gas"`
	MaxTxSigners       uint32                                   `json:"max_tx_signers"`
	MaxMultisigSigners uint32                                   `json:"max_multisig_signers"`
	GasCosts           GasCosts                                 `json:"gas_costs"`
	MinGasPrice        map[types.Denomination]quantity.Quantity `json:"min_gas_price"`
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
