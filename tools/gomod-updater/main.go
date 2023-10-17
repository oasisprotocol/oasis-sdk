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

				fmt.Println("Updating", path)
				if requiresPkg {
					// Update the dependency.
					cmd := exec.Command("go", "get", "-u", pkg+"@v"+version)
					cmd.Dir = filepath.Dir(path)
					cmd.Stdout = os.Stdout
					cmd.Stderr = os.Stderr
					err = cmd.Run()
					if err != nil {
						cobra.CheckErr(fmt.Errorf("failed to update dependency: %w", err))
					}
				}

				// Tidy all projects since a dependency could have been updated
				// in a dependent project (some projects depend on client-sdk).
				cmd := exec.Command("go", "mod", "tidy")
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

	skip      []string
	updateCmd = &cobra.Command{
		Use:     "update-all [--packages <PACKAGE>,...]]",
		Short:   "Utility for updating all go packages in the oasis-sdk repo",
		Version: "0.1.0",
		Run: func(cmd *cobra.Command, args []string) {
			// Go through all packages and their direct dependencies.
			for _, path := range packages {
				data, err := os.ReadFile(path)
				if err != nil {
					cobra.CheckErr(fmt.Errorf("failed to read go.mod file: %w", err))
				}
				file, err := modfile.ParseLax(path, data, nil)
				if err != nil {
					cobra.CheckErr(fmt.Errorf("failed to parse go.mod file: %w", err))
				}
				fmt.Println("Updating packages in:", path)
			OUTER:
				for _, req := range file.Require {
					// Skip indirect dependencies.
					if req.Indirect {
						continue
					}
					for _, s := range skip {
						if req.Mod.Path == s {
							fmt.Println("Skipping", req.Mod.Path)
							continue OUTER
						}
					}

					fmt.Println("Updating...", req.Mod.Path)
					// Update the dependency.
					cmd := exec.Command("go", "get", "-u", req.Mod.Path)
					cmd.Dir = filepath.Dir(path)
					cmd.Stdout = os.Stdout
					cmd.Stderr = os.Stderr
					err = cmd.Run()
					if err != nil {
						cobra.CheckErr(fmt.Errorf("failed to update dependency: %w", err))
					}
				}

				// Tidy the go.mod file after updating all packages.
				fmt.Println("Tidying...", path)
				cmd := exec.Command("go", "mod", "tidy")
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

	updateFlags := flag.NewFlagSet("", flag.ContinueOnError)
	updateFlags.StringSliceVar(&skip, "skip", []string{}, "dependencies to skip")
	updateCmd.Flags().AddFlagSet(updateFlags)
	updateCmd.Flags().AddFlagSet(flags)
	rootCmd.AddCommand(updateCmd)

	_ = rootCmd.Execute()
}
