module github.com/oasisprotocol/oasis-sdk/client-sdk/go

go 1.16

// Should be synced with Oasis Core as replace directives are not propagated.
replace (
	github.com/tendermint/tendermint => github.com/oasisprotocol/tendermint v0.34.9-oasis2
	golang.org/x/crypto/curve25519 => github.com/oasisprotocol/curve25519-voi/primitives/x25519 v0.0.0-20210505121811-294cf0fbfb43
	golang.org/x/crypto/ed25519 => github.com/oasisprotocol/curve25519-voi/primitives/ed25519 v0.0.0-20210505121811-294cf0fbfb43
)

require (
	github.com/btcsuite/btcd v0.22.0-beta
	github.com/oasisprotocol/curve25519-voi v0.0.0-20210716083614-f38f8e8b0b84 // indirect
	github.com/oasisprotocol/oasis-core/go v0.2102.5
	github.com/stretchr/testify v1.7.0
	golang.org/x/net v0.0.0-20210510120150-4163338589ed // indirect
	golang.org/x/sys v0.0.0-20210514084401-e8d321eab015 // indirect
	google.golang.org/grpc v1.39.1
)
