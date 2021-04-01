module github.com/oasisprotocol/oasis-sdk/tests/e2e

go 1.15

// Should be synced with Oasis Core as replace directives are not propagated.
replace (
	github.com/coreos/etcd => github.com/coreos/etcd v3.3.25+incompatible
	github.com/gorilla/websocket => github.com/gorilla/websocket v1.4.2

	// We want to test the current client-sdk.
	github.com/oasisprotocol/oasis-sdk/client-sdk/go => ../../client-sdk/go

	github.com/tendermint/tendermint => github.com/oasisprotocol/tendermint v0.34.8-oasis1
	golang.org/x/crypto/curve25519 => github.com/oasisprotocol/ed25519/extra/x25519 v0.0.0-20210127160119-f7017427c1ea
	golang.org/x/crypto/ed25519 => github.com/oasisprotocol/ed25519 v0.0.0-20210127160119-f7017427c1ea
)

require (
	github.com/oasisprotocol/oasis-core/go v0.2100.1
	github.com/oasisprotocol/oasis-sdk/client-sdk/go v0.0.0-20210328195842-4de788c1c6f7
	google.golang.org/grpc v1.36.0
)
