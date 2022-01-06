package helpers

import (
	"encoding/hex"
	"fmt"
	"strings"

	staking "github.com/oasisprotocol/oasis-core/go/staking/api"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/config"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/modules/rewards"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

const (
	addressPrefixOasis       = "oasis1"
	addressPrefixEth         = "0x"
	addressExplicitSeparator = ":"
	addressExplicitParaTime  = "paratime"
	addressExplicitPool      = "pool"

	poolRewards = "rewards"
)

// ResolveAddress resolves a string address into the corresponding account address.
func ResolveAddress(net *config.Network, address string) (*types.Address, error) {
	switch {
	case strings.HasPrefix(address, addressPrefixOasis):
		// Oasis Bech32 address.
		var a types.Address
		if err := a.UnmarshalText([]byte(address)); err != nil {
			return nil, err
		}
		return &a, nil
	case strings.HasPrefix(address, addressPrefixEth):
		// Ethereum address, derive Oasis Bech32 address.
		ethAddr, err := hex.DecodeString(address[2:])
		if err != nil {
			return nil, fmt.Errorf("malformed Ethereum address: %w", err)
		}
		if len(ethAddr) != 20 {
			return nil, fmt.Errorf("malformed Ethereum address: expected 20 bytes")
		}
		addr := types.NewAddressRaw(types.AddressV0Secp256k1EthContext, ethAddr)
		return &addr, nil
	case strings.Contains(address, addressExplicitSeparator):
		subs := strings.SplitN(address, addressExplicitSeparator, 2)
		switch kind, data := subs[0], subs[1]; kind {
		case addressExplicitParaTime:
			// ParaTime.
			pt := net.ParaTimes.All[data]
			if pt == nil {
				return nil, fmt.Errorf("paratime '%s' does not exist", data)
			}

			addr := types.NewAddressFromConsensus(staking.NewRuntimeAddress(pt.Namespace()))
			return &addr, nil
		case addressExplicitPool:
			// Pool.
			switch data {
			case poolRewards:
				// Reward pool address.
				return &rewards.RewardPoolAddress, nil
			default:
				return nil, fmt.Errorf("unsupported pool kind: %s", data)
			}
		default:
			// Unsupported kind.
			return nil, fmt.Errorf("unsupported explicit address kind: %s", kind)
		}
	default:
		return nil, fmt.Errorf("unsupported address format")
	}
}
