package main

import (
	"fmt"
	"os"
	"os/exec"
	"path/filepath"

	"github.com/spf13/cobra"
	flag "github.com/spf13/pflag"
	"golang.org/x/mod/modfile"
)

var (
	packages []string

	rootCmd = &cobra.Command{
		Use:     "updater <PACKAGE> <VERSION> [--packages <PACKAGE>,...]]",
		Short:   "Utility for updating go packages in the oasis-sdk repo",
		Version: "0.1.0",
		Args:    cobra.ExactArgs(2),
		Run: func(cmd *cobra.Command, args []string) {
			pkg, version := args[0], args[1]

			// Go through all packages and update the dependency (if it exists).
			for _, path := range packages {
				data, err := os.ReadFile(path)
				if err != nil {
					cobra.CheckErr(fmt.Errorf("failed to read go.mod file: %w", err))
				}
				file, err := modfile.ParseLax(path, data, nil)
				if err != nil {
					cobra.CheckErr(fmt.Errorf("failed to parse go.mod file: %w", err))
				}
				var requiresPkg bool
				for _, req := range file.Require {
					if !req.Indirect && req.Mod.Path == pkg {
						requiresPkg = true
						break
					}
				}
				if !requiresPkg {
					// Nothing to do.
					continue
				}
				fmt.Println("Updating", path)

				// Update the dependency.
				cmd := exec.Command("go", "get", "-u", pkg+"@v"+version)
				cmd.Dir = filepath.Dir(path)
				cmd.Stdout = os.Stdout
				cmd.Stderr = os.Stderr
				err = cmd.Run()
				if err != nil {
					cobra.CheckErr(fmt.Errorf("failed to update dependency: %w", err))
				}
				// Tidy.
				cmd = exec.Command("go", "mod", "tidy")
				cmd.Dir = filepath.Dir(path)
				cmd.Stdout = os.Stdout
				cmd.Stderr = os.Stderr
				err = cmd.Run()
				if err != nil {
					cobra.CheckErr(fmt.Errorf("failed to run go mod tidy: %w", err))
				}
			}
		},
	}
)

func main() {
	flags := flag.NewFlagSet("", flag.ContinueOnError)
	flags.StringSliceVar(&packages, "packages", []string{"./go.mod"}, "go.mod files to update")
	rootCmd.Flags().AddFlagSet(flags)

	_ = rootCmd.Execute()
}
