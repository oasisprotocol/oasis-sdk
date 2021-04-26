module github.com/oasisprotocol/oasis-sdk/client-sdk/go

go 1.15

// Should be synced with Oasis Core as replace directives are not propagated.
replace (
	github.com/tendermint/tendermint => github.com/oasisprotocol/tendermint v0.34.9-oasis2
	golang.org/x/crypto/curve25519 => github.com/oasisprotocol/ed25519/extra/x25519 v0.0.0-20210127160119-f7017427c1ea
	golang.org/x/crypto/ed25519 => github.com/oasisprotocol/ed25519 v0.0.0-20210127160119-f7017427c1ea
)

require (
	github.com/btcsuite/btcd v0.21.0-beta
	github.com/oasisprotocol/oasis-core/go v0.2101.1
	github.com/stretchr/testify v1.7.0
	google.golang.org/grpc v1.36.1
)
