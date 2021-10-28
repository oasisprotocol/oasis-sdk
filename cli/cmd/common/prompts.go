package common

import (
	"fmt"

	"github.com/AlecAivazis/survey/v2"
	"github.com/spf13/cobra"
)

var (
	// PromptPassphrase is the standard passphrase prompt.
	PromptPassphrase = &survey.Password{
		Message: "Passphrase:",
	}

	// PromptCreatePassphrase is the standard create a new passphrase prompt.
	PromptCreatePassphrase = &survey.Password{
		Message: "Choose a new passphrase:",
	}

	// PromptRepeatPassphrase is the standard repeat a new passphrase prompt.
	PromptRepeatPassphrase = &survey.Password{
		Message: "Repeat passphrase:",
	}
)

// Confirm asks the user for confirmation and aborts when rejected.
func Confirm(msg, abortMsg string) {
	// TODO: Support flag for skipping confirmations.

	var proceed bool
	err := survey.AskOne(&survey.Confirm{Message: msg}, &proceed)
	cobra.CheckErr(err)
	if !proceed {
		cobra.CheckErr(abortMsg)
	}
}

// AskNewPassphrase asks the user to create a new passphrase.
func AskNewPassphrase() string {
	var answers struct {
		Passphrase  string
		Passphrase2 string
	}
	questions := []*survey.Question{
		{
			Name:   "passphrase",
			Prompt: PromptCreatePassphrase,
		},
		{
			Name:   "passphrase2",
			Prompt: PromptRepeatPassphrase,
			Validate: func(ans interface{}) error {
				if ans.(string) != answers.Passphrase {
					return fmt.Errorf("passphrases do not match")
				}
				return nil
			},
		},
	}
	err := survey.Ask(questions, &answers)
	cobra.CheckErr(err)

	return answers.Passphrase
}
