package inspect

import (
	"context"
	"encoding/json"
	"fmt"
	"strings"

	"github.com/spf13/cobra"

	"github.com/oasisprotocol/oasis-core/go/common/crypto/signature"
	registry "github.com/oasisprotocol/oasis-core/go/registry/api"
	staking "github.com/oasisprotocol/oasis-core/go/staking/api"

	"github.com/oasisprotocol/oasis-sdk/cli/cmd/common"
	cliConfig "github.com/oasisprotocol/oasis-sdk/cli/config"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/connection"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/helpers"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

type registrySelector int

const (
	selInvalid registrySelector = iota
	selEntities
	selNodes
	selRuntimes
	selValidators
)

func selectorFromString(s string) registrySelector {
	switch strings.ToLower(strings.TrimSpace(s)) {
	case "entities":
		return selEntities
	case "nodes":
		return selNodes
	case "runtimes", "paratimes":
		return selRuntimes
	case "validators":
		return selValidators
	}
	return selInvalid
}

var registryCmd = &cobra.Command{
	Use:   "registry <id>",
	Short: "Show registry entry by id",
	Args:  cobra.ExactArgs(1),
	Run: func(cmd *cobra.Command, args []string) {
		cfg := cliConfig.Global()
		npa := common.GetNPASelection(cfg)

		id, err := parseIdentifier(npa, args[0])
		cobra.CheckErr(err)

		// Establish connection with the target network.
		ctx := context.Background()
		conn, err := connection.Connect(ctx, npa.Network)
		cobra.CheckErr(err)

		consensusConn := conn.Consensus()
		registryConn := consensusConn.Registry()

		// Figure out the height to use if "latest".
		height, err := common.GetActualHeight(
			ctx,
			consensusConn,
		)
		cobra.CheckErr(err)

		// This command just takes a brute-force "do-what-I-mean" approach
		// and queries everything it can till it finds what the user is
		// looking for.

		prettyPrint := func(b interface{}) error {
			data, err := json.MarshalIndent(b, "", "  ")
			if err != nil {
				return err
			}
			fmt.Printf("%s\n", data)
			return nil
		}

		switch v := id.(type) {
		case signature.PublicKey:
			idQuery := &registry.IDQuery{
				Height: height,
				ID:     v,
			}

			if entity, err := registryConn.GetEntity(ctx, idQuery); err == nil {
				err = prettyPrint(entity)
				cobra.CheckErr(err)
				return
			}

			if node, err := registryConn.GetNode(ctx, idQuery); err == nil {
				err = prettyPrint(node)
				cobra.CheckErr(err)
				return
			}

			nsQuery := &registry.NamespaceQuery{
				Height: height,
			}
			copy(nsQuery.ID[:], v[:])

			if runtime, err := registryConn.GetRuntime(ctx, nsQuery); err == nil {
				err = prettyPrint(runtime)
				cobra.CheckErr(err)
				return
			}
		case *types.Address:
			addr := staking.Address(*v)

			entities, err := registryConn.GetEntities(ctx, height)
			cobra.CheckErr(err) // If this doesn't work the other large queries won't either.
			for _, entity := range entities {
				if staking.NewAddress(entity.ID).Equal(addr) {
					err = prettyPrint(entity)
					cobra.CheckErr(err)
					return
				}
			}

			nodes, err := registryConn.GetNodes(ctx, height)
			cobra.CheckErr(err)
			for _, node := range nodes {
				if staking.NewAddress(node.ID).Equal(addr) {
					err = prettyPrint(node)
					cobra.CheckErr(err)
					return
				}
			}

			// Probably don't need to bother querying the runtimes by address.
		case registrySelector:
			switch v {
			case selEntities:
				entities, err := registryConn.GetEntities(ctx, height)
				cobra.CheckErr(err)
				for _, entity := range entities {
					err = prettyPrint(entity)
					cobra.CheckErr(err)
				}
				return
			case selNodes:
				nodes, err := registryConn.GetNodes(ctx, height)
				cobra.CheckErr(err)
				for _, node := range nodes {
					err = prettyPrint(node)
					cobra.CheckErr(err)
				}
				return
			case selRuntimes:
				runtimes, err := registryConn.GetRuntimes(ctx, &registry.GetRuntimesQuery{
					Height:           height,
					IncludeSuspended: true,
				})
				cobra.CheckErr(err)
				for _, runtime := range runtimes {
					err = prettyPrint(runtime)
					cobra.CheckErr(err)
				}
				return
			case selValidators:
				// Yes, this is a scheduler query, not a registry query
				// but this also is a reasonable place for this.
				schedulerConn := consensusConn.Scheduler()
				validators, err := schedulerConn.GetValidators(ctx, height)
				cobra.CheckErr(err)
				for _, validator := range validators {
					err = prettyPrint(validator)
					cobra.CheckErr(err)
				}
				return
			default:
				// Should never happen.
			}
		}

		cobra.CheckErr(fmt.Errorf("id '%s' not found", id))
	},
}

func parseIdentifier(
	npa *common.NPASelection,
	s string,
) (interface{}, error) { // TODO: Use `any`
	if sel := selectorFromString(s); sel != selInvalid {
		return sel, nil
	}

	addr, err := helpers.ResolveAddress(npa.Network, s)
	if err == nil {
		return addr, nil
	}

	var pk signature.PublicKey
	if err = pk.UnmarshalText([]byte(s)); err == nil {
		return pk, nil
	}
	if err = pk.UnmarshalHex(s); err == nil {
		return pk, nil
	}

	return nil, fmt.Errorf("unrecognized id: '%s'", s)
}
