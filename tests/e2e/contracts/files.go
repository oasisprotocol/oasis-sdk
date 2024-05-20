package contracts

import (
	_ "embed"
)

//go:embed build/hello.wasm
var helloContractCode []byte

//go:embed build/oas20.wasm
var oas20ContractCode []byte
