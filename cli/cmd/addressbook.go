package cmd

import (
	"fmt"
	"sort"

	"github.com/spf13/cobra"

	"github.com/oasisprotocol/oasis-sdk/cli/config"
	"github.com/oasisprotocol/oasis-sdk/cli/table"
)

var (
	addressBookCmd = &cobra.Command{
		Use:   "addressbook",
		Short: "Manage addresses in the local address book",
	}

	abListCmd = &cobra.Command{
		Use:     "list",
		Aliases: []string{"ls"},
		Short:   "List addresses stored in address book",
		Args:    cobra.NoArgs,
		Run: func(cmd *cobra.Command, args []string) {
			cfg := config.Global()
			table := table.New()
			table.SetHeader([]string{"Name", "Address"})

			var output [][]string
			for name, acc := range cfg.AddressBook.All {
				addrStr := acc.Address
				if ethAddr := acc.GetEthAddress(); ethAddr != nil {
					addrStr = ethAddr.Hex()
				}
				output = append(output, []string{
					name,
					addrStr,
				})
			}

			// Sort output by name.
			sort.Slice(output, func(i, j int) bool {
				return output[i][0] < output[j][0]
			})

			table.AppendBulk(output)
			table.Render()
		},
	}

	abAddCmd = &cobra.Command{
		Use:   "add <name> <address>",
		Short: "Add an address to address book",
		Args:  cobra.ExactArgs(2),
		Run: func(cmd *cobra.Command, args []string) {
			cfg := config.Global()
			name := args[0]
			address := args[1]

			err := cfg.AddressBook.Add(name, address)
			cobra.CheckErr(err)

			err = cfg.Save()
			cobra.CheckErr(err)
		},
	}

	abShowCmd = &cobra.Command{
		Use:   "show <name>",
		Short: "Show address information",
		Args:  cobra.ExactArgs(1),
		Run: func(cmd *cobra.Command, args []string) {
			name := args[0]
			abEntry, ok := config.Global().AddressBook.All[name]
			if !ok {
				cobra.CheckErr(fmt.Errorf("address named '%s' does not exist in the address book", name))
			}

			fmt.Printf("Name:             %s\n", name)
			if abEntry.GetEthAddress() != nil {
				fmt.Printf("Ethereum address: %s\n", abEntry.GetEthAddress().Hex())
			}
			fmt.Printf("Native address:   %s\n", abEntry.GetAddress())
		},
	}

	abRmCmd = &cobra.Command{
		Use:     "rm <name>",
		Aliases: []string{"remove"},
		Short:   "Remove an address from address book",
		Args:    cobra.ExactArgs(1),
		Run: func(cmd *cobra.Command, args []string) {
			cfg := config.Global()
			name := args[0]

			err := cfg.AddressBook.Remove(name)
			cobra.CheckErr(err)

			err = cfg.Save()
			cobra.CheckErr(err)
		},
	}

	abRenameCmd = &cobra.Command{
		Use:   "rename <old> <new>",
		Short: "Rename address",
		Args:  cobra.ExactArgs(2),
		Run: func(cmd *cobra.Command, args []string) {
			cfg := config.Global()
			oldName, newName := args[0], args[1]

			err := cfg.AddressBook.Rename(oldName, newName)
			cobra.CheckErr(err)

			err = cfg.Save()
			cobra.CheckErr(err)
		},
	}
)

func init() {
	addressBookCmd.AddCommand(abAddCmd)
	addressBookCmd.AddCommand(abListCmd)
	addressBookCmd.AddCommand(abRenameCmd)
	addressBookCmd.AddCommand(abRmCmd)
	addressBookCmd.AddCommand(abShowCmd)
}
