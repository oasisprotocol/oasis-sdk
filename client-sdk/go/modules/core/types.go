package core

import (
	"github.com/oasisprotocol/oasis-core/go/common/cbor"
	"github.com/oasisprotocol/oasis-core/go/common/quantity"
	"github.com/oasisprotocol/oasis-core/go/common/version"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

// EstimateGasQuery is the body of the core.EstimateGas query.
type EstimateGasQuery struct {
	// Caller is the address of the caller for which to do estimation. If not specified the
	// authentication information from the passed transaction is used.
	Caller *types.CallerAddress `json:"caller,omitempty"`
	// Tx is the unsigned transaction to estimate.
	Tx *types.Transaction `json:"tx"`
	// PropagateFailures indicates if the estimate gas query should propagate transaction failures.
	PropagateFailures bool `json:"propagate_failures,omitempty"`
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

	// Fields below have omitempty set for backwards compatibility. Once there are no deployed
	// runtimes using an old version of the SDK, this should be removed.

	MaxTxSize uint32 `json:"max_tx_size,omitempty"`
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

// RuntimeInfoResponse is the response of the core.RuntimeInfo query
// and provides basic introspection information about the runtime.
type RuntimeInfoResponse struct {
	RuntimeVersion *version.Version `json:"runtime_version"`
	// StateVersion is the version of the schema used by the runtime for keeping its state.
	StateVersion uint32 `json:"state_version"`
	// Modules are the SDK modules that comprise this runtime.
	Modules map[string]ModuleInfo `json:"modules"`
}

// ModuleInfo is the information about a single module within the runtime.
type ModuleInfo struct {
	// Version is the version of the module.
	Version uint32 `json:"version"`
	// Params are the initial parameters of the module.
	Params cbor.RawMessage `json:"params"`
	// Methods are the RPC methods exposed by the module.
	Methods []MethodHandlerInfo `json:"methods"`
}

// MethodHandlerInfo describes a single RPC.
type MethodHandlerInfo struct {
	// Name is the name of the RPC.
	Name string `json:"name"`
	// Kind is the kind of the RPC.
	Kind methodHandlerKind `json:"kind"`
}

type methodHandlerKind string

// These constants represent the kinds of methods that handlers handle.
const (
	MethodHandlerKindCall          methodHandlerKind = "call"
	MethodHandlerKindQuery         methodHandlerKind = "query"
	MethodHandlerKindMessageResult methodHandlerKind = "message_result"
)

// CallDataPublicKeyResponse is the response of the core.CallDataPublicKey query.
type CallDataPublicKeyResponse struct {
	// PublicKey is the signed runtime call data public key.
	PublicKey types.SignedPublicKey `json:"public_key"`
}

// ExecuteReadOnlyTxQuery is the body of the core.ExecuteReadOnlyTx query.
type ExecuteReadOnlyTxQuery struct {
	Tx []byte `json:"tx"`
}

// ExecuteReadOnlyTxResponse is the response of the core.ExecuteReadOnlyTx query.
type ExecuteReadOnlyTxResponse struct {
	Result types.CallResult `json:"result"`
}
