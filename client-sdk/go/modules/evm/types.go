package evm

// The CreateTx type must match the CreateTx type from the evm module types
// in runtime-sdk/modules/evm/src/types.rs.
type CreateTx struct {
	Value    []byte `json:"value"`
	InitCode []byte `json:"init_code"`
	GasPrice []byte `json:"gas_price"`
	GasLimit uint64 `json:"gas_limit"`
}

// The CallTx type must match the CallTx type from the evm module types
// in runtime-sdk/modules/evm/src/types.rs.
type CallTx struct {
	Address  []byte `json:"address"`
	Value    []byte `json:"value"`
	Data     []byte `json:"data"`
	GasPrice []byte `json:"gas_price"`
	GasLimit uint64 `json:"gas_limit"`
}

// The PeekStorageQuery type must match the PeekStorageQuery type from the
// evm module types in runtime-sdk/modules/evm/src/types.rs.
type PeekStorageQuery struct {
	Address []byte `json:"address"`
	Index   []byte `json:"index"`
}

// The PeekCodeQuery type must match the PeekCodeQuery type from the
// evm module types in runtime-sdk/modules/evm/src/types.rs.
type PeekCodeQuery struct {
	Address []byte `json:"address"`
}
