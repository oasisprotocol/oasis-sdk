# extract-runtime-txs

This tool parses Rust, Go, and TypeScript code and generates markdown docs
describing all runtime transactions with their call types and results.

## Compilation and Testing


```sh
go build
go test ./...
```

## Execution

This should output all runtime transactions in JSON format:

```sh
./extract-runtime-txs \
	--codebase.path ../..
```

For oasis-sdk documentation an existing Markdown template should be used and
any source files should be referenced relative to github.com URL:

```sh
./extract-runtime-txs \
	--codebase.path ../.. \
	--markdown \
	--markdown.template.file ../../docs/runtime/transactions.md.tpl \
	--codebase.url https://github.com/oasisprotocol/oasis-sdk/tree/master/ \
	> ../../docs/runtime/transactions.md
```
