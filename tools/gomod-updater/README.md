# gomod-udpater

### Examples

Update all (direct) dependencies in packages with (skips examples/minimal-runtime-client):

```bash
$ ./tools/gomod-updater/gomod-updater update-all \
	--packages="./client-sdk/go/go.mod,./tests/e2e/go.mod,./tests/benchmark/go.mod,./tools/orc/go.mod,./tools/gomod-updater/go.mod,./tools/gen_runtime_vectors/go.mod,./client-sdk/ts-web/core/reflect-go/go.mod" \
	--skip github.com/oasisprotocol/oasis-sdk/client-sdk/go
```
