package main

import (
	"crypto/rsa"
	"crypto/x509"
	"encoding/pem"
	"fmt"
	"os"
	"strings"

	"github.com/BurntSushi/toml"
	"github.com/spf13/cobra"
	flag "github.com/spf13/pflag"

	"github.com/oasisprotocol/oasis-core/go/common"
	"github.com/oasisprotocol/oasis-core/go/common/sgx"
	"github.com/oasisprotocol/oasis-core/go/common/sgx/sigstruct"
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
	// Init flags.
	sgxExecutableFn   string
	sgxSignatureFn    string
	bundleFn          string
	overrideRuntimeID string

	// SIGSTRUCT flags.
	dateStr                 string
	swdefined               uint32
	isvprodid               uint16
	isvsvn                  uint16
	miscelectMiscmask       string
	xfrm                    string
	attributesAttributemask string
	bit32                   bool
	debug                   bool

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

	sgxGetSignDataCmd = &cobra.Command{
		Use:   "sgx-gen-sign-data <bundle.orc>",
		Short: "outputs the SIGSTRUCT hash that is to be signed in an offline signing process",
		Args:  cobra.ExactArgs(1),
		Run: func(cmd *cobra.Command, args []string) {
			bundlePath := args[0]

			// Load bundle.
			bnd, err := bundle.Open(bundlePath)
			if err != nil {
				cobra.CheckErr(fmt.Errorf("failed to open bundle: %w", err))
			}

			sigstruct := constructSigstruct(bnd)
			fmt.Printf("%s", sigstruct.HashForSignature())
		},
	}
	sgxSetSigCmd = &cobra.Command{
		Use:   "sgx-set-sig <bundle.orc> <signature.sig> <public_key.pub>",
		Short: "add or overwrite an SGXS signature to an existing runtime bundle",
		Args:  cobra.ExactArgs(3),
		Run: func(cmd *cobra.Command, args []string) {
			bundlePath, sigPath, publicKey := args[0], args[1], args[2]

			// Load public key.
			rawPub, err := os.ReadFile(publicKey)
			if err != nil {
				cobra.CheckErr(fmt.Errorf("failed to read public key: %w", err))
			}
			pubPem, _ := pem.Decode(rawPub)
			if pubPem == nil {
				cobra.CheckErr(fmt.Errorf("failed to decode public key pem file"))
			}
			pub, err := x509.ParsePKIXPublicKey(pubPem.Bytes)
			if err != nil {
				cobra.CheckErr(fmt.Errorf("failed to parse public key: %w", err))
			}
			pubKey, ok := pub.(*rsa.PublicKey)
			if !ok {
				cobra.CheckErr(fmt.Errorf("invalid public key type: %T", pub))
			}

			// Load bundle.
			bnd, err := bundle.Open(bundlePath)
			if err != nil {
				cobra.CheckErr(fmt.Errorf("failed to open bundle: %w", err))
			}

			// Load signature file.
			rawSig, err := os.ReadFile(sigPath)
			if err != nil {
				cobra.CheckErr(fmt.Errorf("failed to load signature file: %w", err))
			}

			// Construct sigstruct from provided arguments.
			sigstruct := constructSigstruct(bnd)
			signed, err := sigstruct.WithSignature(rawSig, pubKey)
			if err != nil {
				cobra.CheckErr(fmt.Errorf("failed to append signature: %w", err))
			}
			err = bnd.Add(sgxSigName, signed)
			cobra.CheckErr(err)
			bnd.Manifest.SGX.Signature = sgxSigName

			// Remove previous serialized manifest.
			bnd.ResetManifest()

			// Write the bundle back.
			err = bnd.Write(bundlePath)
			if err != nil {
				cobra.CheckErr(fmt.Errorf("failed to write bundle: %w", err))
			}
		},
	}

	showCmd = &cobra.Command{
		Use:   "show <bundle.orc>",
		Short: "show the content of the runtime bundle",
		Args:  cobra.ExactArgs(1),
		Run: func(cmd *cobra.Command, args []string) {
			bundlePath := args[0]

			// Load bundle.
			bnd, err := bundle.Open(bundlePath)
			if err != nil {
				cobra.CheckErr(fmt.Errorf("failed to open bundle: %w", err))
			}

			fmt.Printf("Bundle:         %s\n", bundlePath)
			fmt.Printf("Name:           %s\n", bnd.Manifest.Name)
			fmt.Printf("Runtime ID:     %s\n", bnd.Manifest.ID)
			fmt.Printf("Version:        %s\n", bnd.Manifest.Version)
			fmt.Printf("Executable:     %s\n", bnd.Manifest.Executable)

			if bnd.Manifest.SGX != nil {
				fmt.Printf("SGXS:           %s\n", bnd.Manifest.SGX.Executable)

				mrEnclave, err := bnd.MrEnclave()
				if err != nil {
					cobra.CheckErr(fmt.Errorf("failed to compute MRENCLAVE: %w", err))
				}
				fmt.Printf("SGXS MRENCLAVE: %s\n", mrEnclave)

				if bnd.Manifest.SGX.Signature != "" {
					fmt.Printf("SGXS signature: %s\n", bnd.Manifest.SGX.Signature)

					_, sigStruct, err := sigstruct.Verify(bnd.Data[bnd.Manifest.SGX.Signature])
					cobra.CheckErr(err) // Already checked during Open so it should never fail.

					fmt.Printf("SGXS SIGSTRUCT:\n")
					fmt.Printf("  Build date:       %s\n", sigStruct.BuildDate)
					fmt.Printf("  MiscSelect:       %08X\n", sigStruct.MiscSelect)
					fmt.Printf("  MiscSelect mask:  %08X\n", sigStruct.MiscSelectMask)
					fmt.Printf("  Attributes flags: %016X\n", sigStruct.Attributes.Flags)

					for _, fm := range []struct {
						flag sgx.AttributesFlags
						name string
					}{
						{sgx.AttributeInit, "init"},
						{sgx.AttributeDebug, "DEBUG"},
						{sgx.AttributeMode64Bit, "64-bit mode"},
						{sgx.AttributeProvisionKey, "provision key"},
						{sgx.AttributeEInitTokenKey, "enclave init token key"},
					} {
						if sigStruct.Attributes.Flags.Contains(fm.flag) {
							fmt.Printf("    - %s\n", fm.name)
						}
					}

					fmt.Printf("  Attributes XFRM:  %016X\n", sigStruct.Attributes.Xfrm)
					fmt.Printf("  Attributes mask:  %016X %016X\n", sigStruct.AttributesMask[0], sigStruct.AttributesMask[1])
					fmt.Printf("  MRENCLAVE:        %s\n", sigStruct.EnclaveHash)
					fmt.Printf("  ISV product ID:   %d\n", sigStruct.ISVProdID)
					fmt.Printf("  ISV SVN:          %d\n", sigStruct.ISVSVN)
				} else {
					fmt.Printf("SGXS signature: [UNSIGNED]\n")
				}
			}

			fmt.Printf("Digests:\n")
			for name, digest := range bnd.Manifest.Digests {
				fmt.Printf("  %s => %s\n", name, digest)
			}
		},
	}
)

func main() {
	_ = rootCmd.Execute()
}

func init() {
	// Init cmd.
	initFlags := flag.NewFlagSet("", flag.ContinueOnError)
	initFlags.StringVar(&sgxExecutableFn, "sgx-executable", "", "SGXS executable for runtimes with TEE support")
	initFlags.StringVar(&sgxSignatureFn, "sgx-signature", "", "detached SGXS signature for runtimes with TEE support")
	initFlags.StringVar(&bundleFn, "output", "", "output bundle filename")
	initFlags.StringVar(&overrideRuntimeID, "runtime-id", "", "override autodetected runtime ID")
	initCmd.Flags().AddFlagSet(initFlags)

	// SGX singing cmds.
	signFlags := flag.NewFlagSet("", flag.ContinueOnError)
	signFlags.StringVar(&dateStr, "date", "", "Sets the SIGSTRUCT DATE field in YYYYMMDD format (default: today)")
	signFlags.Uint32VarP(&swdefined, "swdefined", "s", 0, "Sets the SIGSTRUCT SWDEFINED field")
	signFlags.Uint16VarP(&isvprodid, "isvprodid", "p", 0, "Sets the SIGSTRUCT ISVPRODID field")
	signFlags.Uint16VarP(&isvsvn, "isvsvn", "v", 0, "Sets the SIGSTRUCT ISVSVN field")
	signFlags.StringVarP(&miscelectMiscmask, "miscselect", "m", "0/0", "Sets the MISCSELECT and inverse MISCMASK fields")
	signFlags.StringVarP(&attributesAttributemask, "attributes", "a", "0x4/0x2", "Sets the lower ATTRIBUTES and inverse lower ATTRIBUTEMASK fields")
	signFlags.StringVarP(&xfrm, "xfrm", "x", "0x3/0x3", "Sets the ATTRIBUTES.XFRM and inverse ATTRIBUTEMASK.XFRM fields")
	signFlags.BoolVar(&bit32, "32bit", false, "Unsets the MODE64BIT bit in the ATTRIBUTES field, sets MODE64BIT in the ATTRIBUTEMASK field")
	signFlags.BoolVarP(&debug, "debug", "d", false, "Sets the DEBUG bit in the ATTRIBUTES field, unsets the DEBUG bit in the ATTRIBUTEMASK field")

	sgxGetSignDataCmd.Flags().AddFlagSet(signFlags)
	sgxSetSigCmd.Flags().AddFlagSet(signFlags)

	rootCmd.AddCommand(initCmd)
	rootCmd.AddCommand(sgxGetSignDataCmd)
	rootCmd.AddCommand(sgxSetSigCmd)
	rootCmd.AddCommand(showCmd)
}
