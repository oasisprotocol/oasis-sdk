module github.com/oasisprotocol/oasis-sdk/tests/e2e

go 1.16

// Should be synced with Oasis Core as replace directives are not propagated.
replace (
	github.com/coreos/etcd => github.com/coreos/etcd v3.3.25+incompatible
	// Updates the version used by badgerdb, because some of the Go
	// module caches apparently have a messed up copy that causes
	// build failures.
	// https://github.com/google/flatbuffers/issues/6466
	github.com/google/flatbuffers => github.com/google/flatbuffers v1.12.1
	github.com/gorilla/websocket => github.com/gorilla/websocket v1.4.2

	// We want to test the current client-sdk.
	github.com/oasisprotocol/oasis-sdk/client-sdk/go => ../../client-sdk/go

	github.com/tendermint/tendermint => github.com/oasisprotocol/tendermint v0.34.9-oasis2
	golang.org/x/crypto/curve25519 => github.com/oasisprotocol/curve25519-voi/primitives/x25519 v0.0.0-20210505121811-294cf0fbfb43
	golang.org/x/crypto/ed25519 => github.com/oasisprotocol/curve25519-voi/primitives/ed25519 v0.0.0-20210505121811-294cf0fbfb43
)

require (
	github.com/btcsuite/btcd v0.22.0-beta
	github.com/oasisprotocol/curve25519-voi v0.0.0-20210716083614-f38f8e8b0b84
	github.com/oasisprotocol/oasis-core/go v0.2102.5
	github.com/oasisprotocol/oasis-sdk/client-sdk/go v0.0.0-20210328195842-4de788c1c6f7
	google.golang.org/grpc v1.39.1
)
