all: $(hex)

$(hex): $(contract)
	@solc $(contract) --bin --abi --optimize -o .build
	@mv .build/*.abi $(abi)
	@mv .build/*.bin $(hex)
	@rmdir .build

.PHONY: all

