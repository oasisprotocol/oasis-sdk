package config

// DefaultNetworks is the default config containing known networks.
var DefaultNetworks = Networks{
	Default: "mainnet",
	All: map[string]*Network{
		// Mainnet network parameters.
		// See https://docs.oasis.io/node/mainnet.
		"mainnet": {
			ChainContext: "b11b369e0da5bb230b220127f5e7b242d385ef8c6f54906243f30af63c815535",
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

					// Sapphire on Mainnet.
					"sapphire": {
						ID: "000000000000000000000000000000000000000000000000f80306c9858e7279",
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
		// See https://docs.oasis.io/node/testnet.
		"testnet": {
			ChainContext: "50304f98ddb656620ea817cc1446c401752a05a249b36c9b90dba4616829977a",
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

					// Sapphire on Testnet.
					"sapphire": {
						ID: "000000000000000000000000000000000000000000000000a6d1e3ebf60dff6c",
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
