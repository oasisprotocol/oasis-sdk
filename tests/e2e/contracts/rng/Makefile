contract = evm_rng.sol
abi = evm_rng.abi
hex = evm_rng.hex

all: $(hex)

$(hex): $(contract)
	@solc $(contract) --bin --abi --optimize -o .build
	@mv .build/*.abi $(abi)
	@mv .build/*.bin $(hex)
	@rmdir .build

.PHONY: all

