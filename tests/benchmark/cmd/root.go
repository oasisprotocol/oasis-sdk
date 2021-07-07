// Package cmd implements the commands for the executable.
package cmd

import (
	"fmt"
	"os"

	"github.com/spf13/cobra"
)

var (
	rootCmd = &cobra.Command{
		Use:   "benchmark",
		Short: "Run a benchmark",
		Run:   benchmarkMain,
	}

	fixtureCmd = &cobra.Command{
		Use:   "fixture",
		Short: "dump benchmarking fixture to standard output",
		Run:   doFixture,
	}
)

// Execute spawns the main entry point of the command.
func Execute() {
	if err := rootCmd.Execute(); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}

func init() {
	benchmarkInit(rootCmd)
	fixtureInit(rootCmd)
}
