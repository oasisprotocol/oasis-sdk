module github.com/oasisprotocol/oasis-sdk/client-sdk/go

go 1.16

// Should be synced with Oasis Core as replace directives are not propagated.
replace (
	github.com/tendermint/tendermint => github.com/oasisprotocol/tendermint v0.34.9-oasis2
	golang.org/x/crypto/curve25519 => github.com/oasisprotocol/curve25519-voi/primitives/x25519 v0.0.0-20210505121811-294cf0fbfb43
	golang.org/x/crypto/ed25519 => github.com/oasisprotocol/curve25519-voi/primitives/ed25519 v0.0.0-20210505121811-294cf0fbfb43
)

require (
	github.com/btcsuite/btcd v0.21.0-beta
	github.com/fxamacker/cbor/v2 v2.2.1-0.20210517032302-bdd38cd1c8c0 // indirect
	github.com/oasisprotocol/oasis-core/go v0.2101.1-0.20210517160830-c287752b61b7
	github.com/prometheus/common v0.24.0 // indirect
	github.com/stretchr/testify v1.7.0
	golang.org/x/net v0.0.0-20210510120150-4163338589ed // indirect
	golang.org/x/sys v0.0.0-20210514084401-e8d321eab015 // indirect
	google.golang.org/grpc v1.38.0
)
