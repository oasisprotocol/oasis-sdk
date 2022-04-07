package inspect

import (
	"context"

	"github.com/spf13/cobra"

	consensus "github.com/oasisprotocol/oasis-core/go/consensus/api"

	"github.com/oasisprotocol/oasis-sdk/cli/cmd/common"
)

// Cmd is the network inspection sub-command set root.
var Cmd = &cobra.Command{
	Use:   "inspect",
	Short: "Inspect the network",
}

func getActualHeight(
	ctx context.Context,
	consensusConn consensus.ClientBackend,
	height int64,
) int64 {
	if height != consensus.HeightLatest {
		return height
	}

	blk, err := consensusConn.GetBlock(ctx, height)
	cobra.CheckErr(err)

	return blk.Height
}

func init() {
	governanceProposalCmd.Flags().AddFlagSet(common.SelectorFlags)
	governanceProposalCmd.Flags().AddFlagSet(common.HeightFlag)

	runtimeStatsCmd.Flags().AddFlagSet(common.SelectorFlags)

	nativeTokenCmd.Flags().AddFlagSet(common.SelectorFlags)
	nativeTokenCmd.Flags().AddFlagSet(common.HeightFlag)

	Cmd.AddCommand(governanceProposalCmd)
	Cmd.AddCommand(runtimeStatsCmd)
	Cmd.AddCommand(nativeTokenCmd)
}
