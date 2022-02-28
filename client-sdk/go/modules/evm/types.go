package evm

// The types in this file must match the types from the evm module types
// in runtime-sdk/modules/evm/src/types.rs.

// Create is an EVM CREATE transaction.
type Create struct {
	Value    []byte `json:"value"`
	InitCode []byte `json:"init_code"`
}

// Call is an EVM CALL transaction.
type Call struct {
	Address []byte `json:"address"`
	Value   []byte `json:"value"`
	Data    []byte `json:"data"`
}

// StorageQuery queries the EVM storage.
type StorageQuery struct {
	Address []byte `json:"address"`
	Index   []byte `json:"index"`
}

// CodeQuery queries the EVM code storage.
type CodeQuery struct {
	Address []byte `json:"address"`
}

// BalanceQuery queries the EVM account balance.
type BalanceQuery struct {
	Address []byte `json:"address"`
}

// SimulateCallQuery simulates an EVM CALL.
type SimulateCallQuery struct {
	GasPrice []byte `json:"gas_price"`
	GasLimit uint64 `json:"gas_limit"`
	Caller   []byte `json:"caller"`
	Address  []byte `json:"address"`
	Value    []byte `json:"value"`
	Data     []byte `json:"data"`
}

// GasCosts are the EVM module gas costs.
type GasCosts struct{}

// Parameters are the parameters for the EVM module.
type Parameters struct {
	GasCosts GasCosts `json:"gas_costs"`
}

// ModuleName is the EVM module name.
const ModuleName = "evm"

// Event is an event emitted by the EVM module.
type Event struct {
	Address []byte   `json:"address"`
	Topics  [][]byte `json:"topics"`
	Data    []byte   `json:"data"`
}
