package inspect

import (
	"context"
	"fmt"

	"github.com/spf13/cobra"

	"github.com/oasisprotocol/oasis-sdk/cli/cmd/common"
	cliConfig "github.com/oasisprotocol/oasis-sdk/cli/config"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/connection"
)

var nodeStatusCmd = &cobra.Command{
	Use:   "node-status",
	Short: "Show node status",
	Args:  cobra.NoArgs,
	Run: func(cmd *cobra.Command, args []string) {
		cfg := cliConfig.Global()
		npa := common.GetNPASelection(cfg)

		// Establish connection with the target network.
		ctx := context.Background()
		conn, err := connection.Connect(ctx, npa.Network)
		cobra.CheckErr(err)

		ctrlConn := conn.Control()

		nodeStatus, err := ctrlConn.GetStatus(ctx)
		cobra.CheckErr(err)

		nodeStr, err := common.PrettyJSONMarshal(nodeStatus)
		cobra.CheckErr(err)

		fmt.Println(string(nodeStr))
	},
}
