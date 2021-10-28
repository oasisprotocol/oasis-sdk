package common

import (
	"fmt"

	"github.com/AlecAivazis/survey/v2"
	"github.com/spf13/cobra"

	"github.com/oasisprotocol/oasis-sdk/cli/config"
	"github.com/oasisprotocol/oasis-sdk/cli/wallet"
)

// LoadWallet loads the given named wallet.
func LoadWallet(cfg *config.Config, name string) wallet.Wallet {
	// Early check for whether the wallet exists so that we don't ask for passphrase first.
	var (
		wcfg   *config.Wallet
		exists bool
	)
	if wcfg, exists = cfg.Wallets.All[name]; !exists {
		cobra.CheckErr(fmt.Errorf("wallet '%s' does not exist", name))
	}

	wf, err := wcfg.LoadFactory()
	cobra.CheckErr(err)

	var passphrase string
	if wf.RequiresPassphrase() {
		// Ask for passphrase to decrypt the wallet.
		fmt.Printf("Unlock your wallet.\n")

		err = survey.AskOne(PromptPassphrase, &passphrase)
		cobra.CheckErr(err)
	}

	wallet, err := cfg.Wallets.Load(name, passphrase)
	cobra.CheckErr(err)

	return wallet
}
