package common

import (
	"fmt"

	"github.com/spf13/cobra"
	flag "github.com/spf13/pflag"

	cliConfig "github.com/oasisprotocol/oasis-sdk/cli/config"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/config"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/helpers"
)

var (
	selectedNetwork  string
	selectedParaTime string
	selectedAccount  string

	noParaTime bool
)

// SelectorFlags contains the common selector flags for network/paratime/wallet.
var SelectorFlags *flag.FlagSet

// NPASelection contains the network/paratime/account selection.
type NPASelection struct {
	NetworkName string
	Network     *config.Network

	ParaTimeName string
	ParaTime     *config.ParaTime

	AccountName string
	Account     *cliConfig.Account
}

// GetNPASelection returns the user-selected network/paratime/account combination.
func GetNPASelection(cfg *cliConfig.Config) *NPASelection {
	var s NPASelection
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

	s.AccountName = cfg.Wallet.Default
	if selectedAccount != "" {
		s.AccountName = selectedAccount
	}
	if s.AccountName != "" {
		if testName := helpers.ParseTestAccountAddress(s.AccountName); testName != "" {
			testAcc, err := LoadTestAccountConfig(testName)
			cobra.CheckErr(err)
			s.Account = testAcc
		} else {
			s.Account = cfg.Wallet.All[s.AccountName]
			if s.Account == nil {
				cobra.CheckErr(fmt.Errorf("account '%s' does not exist in the wallet", s.AccountName))
			}
		}
	}

	return &s
}

func init() {
	SelectorFlags = flag.NewFlagSet("", flag.ContinueOnError)
	SelectorFlags.StringVar(&selectedNetwork, "network", "", "explicitly set network to use")
	SelectorFlags.StringVar(&selectedParaTime, "paratime", "", "explicitly set paratime to use")
	SelectorFlags.BoolVar(&noParaTime, "no-paratime", false, "explicitly set that no paratime should be used")
	SelectorFlags.StringVar(&selectedAccount, "account", "", "explicitly set account to use")

	// Backward compatibility.
	SelectorFlags.StringVar(&selectedAccount, "wallet", "", "explicitly set account to use. OBSOLETE, USE --account INSTEAD!")
	err := SelectorFlags.MarkHidden("wallet")
	cobra.CheckErr(err)
}
