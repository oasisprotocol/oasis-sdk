module github.com/oasisprotocol/oasis-sdk/tests/benchmark

go 1.16

replace (
	// Should be synced with Oasis Core as replace directives are not propagated.
	github.com/coreos/etcd => github.com/coreos/etcd v3.3.25+incompatible
	github.com/gorilla/websocket => github.com/gorilla/websocket v1.4.2

	// We want to test the current client-sdk.
	github.com/oasisprotocol/oasis-sdk/client-sdk/go => ../../client-sdk/go
	github.com/tendermint/tendermint => github.com/oasisprotocol/tendermint v0.34.9-oasis2
	golang.org/x/crypto/curve25519 => github.com/oasisprotocol/curve25519-voi/primitives/x25519 v0.0.0-20210505121811-294cf0fbfb43
	golang.org/x/crypto/ed25519 => github.com/oasisprotocol/curve25519-voi/primitives/ed25519 v0.0.0-20210505121811-294cf0fbfb43
)

require (
	github.com/oasisprotocol/oasis-core/go v0.2102.5
	github.com/oasisprotocol/oasis-sdk/client-sdk/go v0.0.0-00010101000000-000000000000
	github.com/prometheus/client_golang v1.11.0
	github.com/spf13/cobra v1.1.3
	github.com/spf13/pflag v1.0.5
	github.com/spf13/viper v1.7.1
	google.golang.org/grpc v1.38.0
)
