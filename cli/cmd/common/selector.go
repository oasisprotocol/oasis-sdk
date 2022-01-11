package common

import (
	"fmt"

	"github.com/spf13/cobra"
	flag "github.com/spf13/pflag"

	cliConfig "github.com/oasisprotocol/oasis-sdk/cli/config"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/config"
)

var (
	selectedNetwork  string
	selectedParaTime string
	selectedWallet   string

	noParaTime bool
)

// SelectorFlags contains the common selector flags for network/paratime/wallet.
var SelectorFlags *flag.FlagSet

// NPWSelection contains the network/paratime/wallet selection.
type NPWSelection struct {
	NetworkName string
	Network     *config.Network

	ParaTimeName string
	ParaTime     *config.ParaTime

	WalletName string
	Wallet     *cliConfig.Wallet
}

// GetNPWSelection returns the user-selected network/paratime/wallet combination.
func GetNPWSelection(cfg *cliConfig.Config) *NPWSelection {
	var s NPWSelection
	s.NetworkName = cfg.Networks.Default
	if selectedNetwork != "" {
		s.NetworkName = selectedNetwork
	}
	if s.NetworkName == "" {
		cobra.CheckErr(fmt.Errorf("no networks configured"))
	}
	s.Network = cfg.Networks.All[s.NetworkName]
	if s.Network == nil {
		cobra.CheckErr(fmt.Errorf("network '%s' does not exist", s.NetworkName))
	}

	if !noParaTime {
		s.ParaTimeName = s.Network.ParaTimes.Default
		if selectedParaTime != "" {
			s.ParaTimeName = selectedParaTime
		}
		if s.ParaTimeName != "" {
			s.ParaTime = s.Network.ParaTimes.All[s.ParaTimeName]
			if s.ParaTime == nil {
				cobra.CheckErr(fmt.Errorf("paratime '%s' does not exist", s.ParaTimeName))
			}
		}
	}

	s.WalletName = cfg.Wallets.Default
	if selectedWallet != "" {
		s.WalletName = selectedWallet
	}
	if s.WalletName != "" {
		s.Wallet = cfg.Wallets.All[s.WalletName]
		if s.Wallet == nil {
			cobra.CheckErr(fmt.Errorf("wallet '%s' does not exist", s.WalletName))
		}
	}

	return &s
}

func init() {
	SelectorFlags = flag.NewFlagSet("", flag.ContinueOnError)
	SelectorFlags.StringVar(&selectedNetwork, "network", "", "explicitly set network to use")
	SelectorFlags.StringVar(&selectedParaTime, "paratime", "", "explicitly set paratime to use")
	SelectorFlags.BoolVar(&noParaTime, "no-paratime", false, "explicitly set that no paratime should be used")
	SelectorFlags.StringVar(&selectedWallet, "wallet", "", "explicitly set wallet to use")
}
