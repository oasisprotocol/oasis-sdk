package config

// DefaultNetworks is the default config containing known networks.
var DefaultNetworks = Networks{
	Default: "mainnet",
	All: map[string]*Network{
		// Mainnet network parameters.
		// See https://docs.oasis.io/node/mainnet.
		"mainnet": {
			ChainContext: "bb3d748def55bdfb797a2ac53ee6ee141e54cd2ab2dc2375f4a0703a178e6e55",
			RPC:          "grpc.oasis.io:443",
			Denomination: DenominationInfo{
				Symbol:   "ROSE",
				Decimals: 9,
			},
			ParaTimes: ParaTimes{
				Default: "sapphire",
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
						ConsensusDenomination: NativeDenominationKey,
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
						ConsensusDenomination: NativeDenominationKey,
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
						ConsensusDenomination: NativeDenominationKey,
					},
				},
			},
		},
		// Oasis Protocol Foundation Testnet parameters.
		// See https://docs.oasis.io/node/testnet.
		"testnet": {
			ChainContext: "0b91b8e4e44b2003a7c5e23ddadb5e14ef5345c0ebcb3ddcae07fa2f244cab76",
			RPC:          "testnet.grpc.oasis.io:443",
			Denomination: DenominationInfo{
				Symbol:   "TEST",
				Decimals: 9,
			},
			ParaTimes: ParaTimes{
				Default: "sapphire",
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
						ConsensusDenomination: NativeDenominationKey,
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
						ConsensusDenomination: NativeDenominationKey,
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
						ConsensusDenomination: NativeDenominationKey,
					},

					// Pontus-X Devnet on Testnet.
					"pontusx_dev": {
						Description: "Pontus-X Devnet",
						ID:          "0000000000000000000000000000000000000000000000004febe52eb412b421",
						Denominations: map[string]*DenominationInfo{
							NativeDenominationKey: {
								Symbol:   "EUROe",
								Decimals: 18,
							},
							// The consensus layer denomination when deposited into the runtime.
							"TEST": {
								Symbol:   "TEST",
								Decimals: 18,
							},
						},
						ConsensusDenomination: "TEST",
					},

					// Pontus-X Testnet on Testnet.
					"pontusx_test": {
						Description: "Pontus-X Testnet",
						ID:          "00000000000000000000000000000000000000000000000004a6f9071c007069",
						Denominations: map[string]*DenominationInfo{
							NativeDenominationKey: {
								Symbol:   "EUROe",
								Decimals: 18,
							},
							// The consensus layer denomination when deposited into the runtime.
							"TEST": {
								Symbol:   "TEST",
								Decimals: 18,
							},
						},
						ConsensusDenomination: "TEST",
					},
				},
			},
		},
	},
}
