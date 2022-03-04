package main

import (
	"fmt"
	"os"
	"strings"

	"github.com/BurntSushi/toml"
	"github.com/spf13/cobra"
	flag "github.com/spf13/pflag"

	"github.com/oasisprotocol/oasis-core/go/common"
	"github.com/oasisprotocol/oasis-core/go/common/version"
	"github.com/oasisprotocol/oasis-core/go/runtime/bundle"
)

const (
	cargoTomlName = "Cargo.toml"

	execName    = "runtime.elf"
	sgxExecName = "runtime.sgx"
	sgxSigName  = "runtime.sgx.sig"
)

var (
	sgxExecutableFn   string
	sgxSignatureFn    string
	bundleFn          string
	overrideRuntimeID string

	rootCmd = &cobra.Command{
		Use:     "orc",
		Short:   "Utility for manipulating Oasis Runtime Containers",
		Version: "0.1.0",
	}

	initCmd = &cobra.Command{
		Use:   "init <ELF-executable> [--sgx-executable SGXS] [--sgx-signature SIG]",
		Short: "create a runtime bundle",
		Args:  cobra.ExactArgs(1),
		Run: func(cmd *cobra.Command, args []string) {
			executablePath := args[0]

			manifest := &bundle.Manifest{
				Executable: execName,
			}

			// Parse Cargo manifest to get name and version.
			data, err := os.ReadFile(cargoTomlName)
			if err != nil {
				cobra.CheckErr(fmt.Errorf("failed to read Cargo manifest: %w", err))
			}

			type deploymentManifest struct {
				RuntimeID common.Namespace `toml:"runtime-id"`
			}
			type cargoManifest struct {
				Package struct {
					Name     string `toml:"name"`
					Version  string `toml:"version"`
					Metadata struct {
						ORC struct {
							Release *deploymentManifest `toml:"release"`
							Test    *deploymentManifest `toml:"test"`
						} `toml:"orc"`
					} `toml:"metadata"`
				} `toml:"package"`
			}
			var cm cargoManifest
			err = toml.Unmarshal(data, &cm)
			if err != nil {
				cobra.CheckErr(fmt.Errorf("malformed Cargo manifest: %w", err))
			}

			manifest.Name = cm.Package.Name
			manifest.Version, err = version.FromString(cm.Package.Version)
			if err != nil {
				cobra.CheckErr(fmt.Errorf("malformed runtime version: %w", err))
			}

			var kind string
			switch overrideRuntimeID {
			case "":
				// Automatic runtime ID determination based on the version string.
				var dm *deploymentManifest
				switch isRelease := (manifest.Version.String() == cm.Package.Version); isRelease {
				case true:
					// Release build.
					dm = cm.Package.Metadata.ORC.Release
					kind = "release"
				case false:
					// Test build.
					dm = cm.Package.Metadata.ORC.Test
					kind = "test"
				}
				if dm == nil {
					cobra.CheckErr(fmt.Errorf("missing ORC metadata for %s build", kind))
				}

				manifest.ID = dm.RuntimeID
			default:
				// Manually configured runtime ID.
				kind = "manually overriden"
				err = manifest.ID.UnmarshalText([]byte(overrideRuntimeID))
				if err != nil {
					cobra.CheckErr(fmt.Errorf("malformed runtime identifier: %w", err))
				}
			}

			fmt.Printf("Using %s runtime identifier: %s\n", strings.ToUpper(kind), manifest.ID)

			// Collect all assets for the bundle.
			type runtimeFile struct {
				fn, descr, dst string
			}
			wantFiles := []runtimeFile{
				{
					fn:    executablePath,
					descr: "runtime ELF binary",
					dst:   execName,
				},
			}
			if sgxExecutableFn != "" {
				wantFiles = append(wantFiles, runtimeFile{
					fn:    sgxExecutableFn,
					descr: "runtime SGX binary",
					dst:   sgxExecName,
				})
				manifest.SGX = &bundle.SGXMetadata{
					Executable: sgxExecName,
				}

				if sgxSignatureFn != "" {
					wantFiles = append(wantFiles, runtimeFile{
						fn:    sgxSignatureFn,
						descr: "runtime SGX signature",
						dst:   sgxSigName,
					})
					manifest.SGX.Signature = sgxSigName
				}
			}

			// Build the bundle.
			bnd := &bundle.Bundle{
				Manifest: manifest,
			}
			for _, v := range wantFiles {
				if v.fn == "" {
					cobra.CheckErr(fmt.Errorf("missing runtime asset '%s'", v.descr))
				}
				var b []byte
				if b, err = os.ReadFile(v.fn); err != nil {
					cobra.CheckErr(fmt.Errorf("failed to load runtime asset '%s': %w", v.descr, err))
				}
				_ = bnd.Add(v.dst, b)
			}

			// Validate SGXS signature (if any).
			if manifest.SGX != nil && manifest.SGX.Signature != "" {
				err = sgxVerifySignature(bnd)
				cobra.CheckErr(err)
			}

			// Write the bundle out.
			outFn := fmt.Sprintf("%s.orc", manifest.Name)
			if bundleFn != "" {
				outFn = bundleFn
			}
			if err = bnd.Write(outFn); err != nil {
				cobra.CheckErr(fmt.Errorf("failed to write output bundle: %w", err))
			}
		},
	}

	sgxSetSigCmd = &cobra.Command{
		Use:   "sgx-set-sig <bundle.orc> <signature.sig>",
		Short: "add or overwrite an SGXS signature in an existing runtime bundle",
		Args:  cobra.ExactArgs(2),
		Run: func(cmd *cobra.Command, args []string) {
			bundlePath, sigPath := args[0], args[1]

			// Load bundle.
			bnd, err := bundle.Open(bundlePath)
			if err != nil {
				cobra.CheckErr(fmt.Errorf("failed to open bundle: %w", err))
			}

			// Load signature file.
			data, err := os.ReadFile(sigPath)
			if err != nil {
				cobra.CheckErr(fmt.Errorf("failed to load signature file: %w", err))
			}

			_ = bnd.Add(sgxSigName, data)
			bnd.Manifest.SGX.Signature = sgxSigName

			// Validate signature.
			err = sgxVerifySignature(bnd)
			cobra.CheckErr(err)

			// Remove previous serialized manifest.
			// TODO: Manifest name should be exposed or there should be a method for clearing it.
			delete(bnd.Data, "META-INF/MANIFEST.MF")

			// Write the bundle back.
			// TODO: Could be more careful and not overwrite.
			err = bnd.Write(bundlePath)
			if err != nil {
				cobra.CheckErr(fmt.Errorf("failed to write bundle: %w", err))
			}
		},
	}
)

func main() {
	_ = rootCmd.Execute()
}

func init() {
	initFlags := flag.NewFlagSet("", flag.ContinueOnError)
	initFlags.StringVar(&sgxExecutableFn, "sgx-executable", "", "SGXS executable for runtimes with TEE support")
	initFlags.StringVar(&sgxSignatureFn, "sgx-signature", "", "detached SGXS signature for runtimes with TEE support")
	initFlags.StringVar(&bundleFn, "output", "", "output bundle filename")
	initFlags.StringVar(&overrideRuntimeID, "runtime-id", "", "override autodetected runtime ID")
	initCmd.Flags().AddFlagSet(initFlags)

	rootCmd.AddCommand(initCmd)
	rootCmd.AddCommand(sgxSetSigCmd)
}
