package common

import (
	"fmt"

	"github.com/AlecAivazis/survey/v2"
	"github.com/spf13/cobra"

	"github.com/oasisprotocol/oasis-sdk/cli/config"
	"github.com/oasisprotocol/oasis-sdk/cli/wallet"
)

// LoadAccount loads the given named account.
func LoadAccount(cfg *config.Config, name string) wallet.Account {
	// Early check for whether the account exists so that we don't ask for passphrase first.
	var (
		acfg   *config.Account
		exists bool
	)
	if acfg, exists = cfg.Wallet.All[name]; !exists {
		cobra.CheckErr(fmt.Errorf("account '%s' does not exist in the wallet", name))
	}

	af, err := acfg.LoadFactory()
	cobra.CheckErr(err)

	var passphrase string
	if af.RequiresPassphrase() {
		// Ask for passphrase to decrypt the account.
		fmt.Printf("Unlock your account.\n")

		err = survey.AskOne(PromptPassphrase, &passphrase)
		cobra.CheckErr(err)
	}

	acc, err := cfg.Wallet.Load(name, passphrase)
	cobra.CheckErr(err)

	return acc
}
