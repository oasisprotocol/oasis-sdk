package inspect

import (
	"github.com/spf13/cobra"

	"github.com/oasisprotocol/oasis-sdk/cli/cmd/common"
)

// Cmd is the network inspection sub-command set root.
var Cmd = &cobra.Command{
	Use:   "inspect",
	Short: "Inspect the network",
}

func init() {
	governanceProposalCmd.Flags().AddFlagSet(common.SelectorFlags)
	governanceProposalCmd.Flags().AddFlagSet(common.HeightFlag)

	runtimeStatsCmd.Flags().AddFlagSet(common.SelectorFlags)
	runtimeStatsCmd.Flags().AddFlagSet(csvFlags)

	nativeTokenCmd.Flags().AddFlagSet(common.SelectorFlags)
	nativeTokenCmd.Flags().AddFlagSet(common.HeightFlag)

	nodeStatusCmd.Flags().AddFlagSet(common.SelectorFlags)

	Cmd.AddCommand(governanceProposalCmd)
	Cmd.AddCommand(runtimeStatsCmd)
	Cmd.AddCommand(nativeTokenCmd)
	Cmd.AddCommand(nodeStatusCmd)
}
