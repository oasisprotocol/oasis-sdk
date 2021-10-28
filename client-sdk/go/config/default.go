package config

// DefaultNetworks is the default config containing known networks.
var DefaultNetworks = Networks{
	Default: "mainnet",
	All: map[string]*Network{
		// Mainnet network parameters.
		// See https://docs.oasis.dev/general/oasis-network/network-parameters.
		"mainnet": {
			ChainContext: "53852332637bacb61b91b6411ab4095168ba02a50be4c3f82448438826f23898",
			RPC:          "grpc.oasis.dev:443",
			Denomination: DenominationInfo{
				Symbol:   "ROSE",
				Decimals: 9,
			},
			ParaTimes: ParaTimes{
				Default: "emerald",
				All: map[string]*ParaTime{
					// Cipher on Mainnet.
					"cipher": {
						ID: "000000000000000000000000000000000000000000000000e199119c992377cb",
						Denominations: map[string]*DenominationInfo{
							NativeDenominationKey: {
								Symbol:   "ROSE",
								Decimals: 9,
							},
						},
					},

					// Emerald on Mainnet.
					"emerald": {
						ID: "000000000000000000000000000000000000000000000000e2eaa99fc008f87f",
						Denominations: map[string]*DenominationInfo{
							NativeDenominationKey: {
								Symbol:   "ROSE",
								Decimals: 18,
							},
						},
					},
				},
			},
		},
		// Oasis Protocol Foundation Testnet parameters.
		// See https://docs.oasis.dev/general/foundation/testnet.
		"testnet": {
			ChainContext: "5ba68bc5e01e06f755c4c044dd11ec508e4c17f1faf40c0e67874388437a9e55",
			RPC:          "testnet.grpc.oasis.dev:443",
			Denomination: DenominationInfo{
				Symbol:   "TEST",
				Decimals: 9,
			},
			ParaTimes: ParaTimes{
				Default: "emerald",
				All: map[string]*ParaTime{
					// Cipher on Testnet.
					"cipher": {
						ID: "0000000000000000000000000000000000000000000000000000000000000000",
						Denominations: map[string]*DenominationInfo{
							NativeDenominationKey: {
								Symbol:   "TEST",
								Decimals: 9,
							},
						},
					},

					// Emerald on Testnet.
					"emerald": {
						ID: "00000000000000000000000000000000000000000000000072c8215e60d5bca7",
						Denominations: map[string]*DenominationInfo{
							NativeDenominationKey: {
								Symbol:   "TEST",
								Decimals: 18,
							},
						},
					},
				},
			},
		},
	},
}
