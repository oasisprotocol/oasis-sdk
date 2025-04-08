all: $(hex)

clean:
	@rm -f $(abi) $(hex)

$(hex): $(contract)
	@solc $(contract) --evm-version paris --bin --abi --optimize -o .build
	@mv .build/*.abi $(abi)
	@mv .build/*.bin $(hex)
	@rmdir .build

.PHONY: all clean
