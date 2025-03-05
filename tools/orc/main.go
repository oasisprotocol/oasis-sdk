package main

import (
	"crypto/rand"
	"crypto/rsa"
	"crypto/x509"
	"encoding/pem"
	"fmt"
	"io"
	"math/big"
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
	"github.com/oasisprotocol/oasis-core/go/runtime/bundle/component"
)

const (
	cargoTomlName = "Cargo.toml"

	execNameFmt    = "%s.elf"
	sgxExecNameFmt = "%s.sgx"
	sgxSigNameFmt  = "%s.sgx.sig"
)

var (
	// Init flags.
	noAutodetection        bool
	sgxExecutableFn        string
	sgxSignatureFn         string
	bundleFn               string
	overrideRuntimeName    string
	overrideRuntimeID      string
	overrideRuntimeVersion string
	componentId            string

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
		Version: "0.3.0",
	}

	initCmd = &cobra.Command{
		Use:   "init [<ELF-executable>] [--sgx-executable SGXS] [--sgx-signature SIG]",
		Short: "create a runtime bundle (optionally with a RONL component)",
		Args:  cobra.MaximumNArgs(1),
		Run: func(cmd *cobra.Command, args []string) {
			var executablePath string
			if len(args) >= 1 {
				executablePath = args[0]
			}

			manifest := autodetectRuntime()

			bnd := &bundle.Bundle{
				Manifest: manifest,
			}

			if executablePath != "" {
				addComponent(bnd, component.ID_RONL, executablePath)
			}
			writeBundle(bnd)
		},
	}

	compAddCmd = &cobra.Command{
		Use:   "component-add <bundle.orc> COMP-ID <ELF-executable> [--sgx-executable SGXS] [--sgx-signature SIG]",
		Short: "adds a new component to an existing runtime bundle",
		Args:  cobra.ExactArgs(3),
		Run: func(cmd *cobra.Command, args []string) {
			bundlePath, rawId, executablePath := args[0], args[1], args[2]

			var compId component.ID
			err := compId.UnmarshalText([]byte(rawId))
			cobra.CheckErr(err)

			// Load bundle.
			bnd, err := bundle.Open(bundlePath)
			if err != nil {
				cobra.CheckErr(fmt.Errorf("failed to open bundle: %w", err))
			}

			addComponent(bnd, compId, executablePath)
			writeBundle(bnd)
		},
	}

	sgxGetSignDataCmd = &cobra.Command{
		Use:   "sgx-gen-sign-data [--component ID] <bundle.orc>",
		Short: "outputs the SIGSTRUCT hash that is to be signed in an offline signing process",
		Args:  cobra.ExactArgs(1),
		Run: func(cmd *cobra.Command, args []string) {
			bundlePath := args[0]
			compId := getComponentID()

			// Load bundle.
			bnd, err := bundle.Open(bundlePath)
			if err != nil {
				cobra.CheckErr(fmt.Errorf("failed to open bundle: %w", err))
			}

			sigstruct := constructSigstruct(bnd, compId)
			fmt.Printf("%s", sigstruct.HashForSignature())
		},
	}

	sgxSetSigCmd = &cobra.Command{
		Use:   "sgx-set-sig [--component ID] <bundle.orc> [<signature.sig> <public_key.pub>]",
		Short: "add or overwrite an SGXS signature to an existing runtime bundle",
		Args:  cobra.RangeArgs(1, 3),
		Run: func(cmd *cobra.Command, args []string) {
			var sigPath, publicKey string
			bundlePath := args[0]
			switch len(args) {
			case 1:
			case 3:
				sigPath, publicKey = args[1], args[2]
			default:
				cobra.CheckErr("unsupported number of arguments")
			}
			compId := getComponentID()

			rawCompId, _ := compId.MarshalText()
			sgxSigName := fmt.Sprintf(sgxSigNameFmt, string(rawCompId))

			// Load bundle.
			bnd, err := bundle.Open(bundlePath)
			if err != nil {
				cobra.CheckErr(fmt.Errorf("failed to open bundle: %w", err))
			}

			// Construct sigstruct from provided arguments.
			var signed []byte
			sigstruct := constructSigstruct(bnd, compId)
			switch sigPath {
			case "":
				// Generate a new random key and sign the sigstruct.
				sigKey, err := sgxGenerateKey(rand.Reader)
				if err != nil {
					cobra.CheckErr(fmt.Errorf("failed to generate signer key: %w", err))
				}
				signed, err = sigstruct.Sign(sigKey)
				if err != nil {
					cobra.CheckErr(fmt.Errorf("failed to sign SIGSTRUCT: %w", err))
				}
			default:
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

				// Load signature file.
				rawSig, err := os.ReadFile(sigPath)
				if err != nil {
					cobra.CheckErr(fmt.Errorf("failed to load signature file: %w", err))
				}

				signed, err = sigstruct.WithSignature(rawSig, pubKey)
				if err != nil {
					cobra.CheckErr(fmt.Errorf("failed to append signature: %w", err))
				}
			}
			err = bnd.Add(sgxSigName, bundle.NewBytesData(signed))
			cobra.CheckErr(err)

			switch compId {
			case component.ID_RONL:
				// We need to support legacy manifests, so check where the SGXS is defined.
				if bnd.Manifest.SGX != nil {
					bnd.Manifest.SGX.Signature = sgxSigName
					break
				}

				fallthrough
			default:
				// Configure SGX signature for the right component.
				comp, ok := bnd.Manifest.GetComponentByID(compId)
				if !ok {
					cobra.CheckErr(fmt.Errorf("component '%s' does not exist", compId))
				}
				comp.SGX.Signature = sgxSigName
			}

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

			fmt.Printf("Components:\n")
			if bnd.Manifest.Executable != "" {
				legacyRonlComp, _ := bnd.Manifest.GetComponentByID(component.ID_RONL)
				showComponent(bnd, legacyRonlComp, true)
			}

			for _, comp := range bnd.Manifest.Components {
				showComponent(bnd, comp, false)
			}

			fmt.Printf("Digests:\n")
			for name, digest := range bnd.Manifest.Digests {
				fmt.Printf("  %s => %s\n", name, digest)
			}
		},
	}
)

func autodetectRuntime() *bundle.Manifest {
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
	switch noAutodetection {
	case true:
		// Manual, ensure all overrides are set.
		if overrideRuntimeName == "" {
			cobra.CheckErr(fmt.Errorf("manual configuration requires --runtime-name"))
		}
		if overrideRuntimeID == "" {
			cobra.CheckErr(fmt.Errorf("manual configuration requires --runtime-id"))
		}
		if overrideRuntimeVersion == "" {
			cobra.CheckErr(fmt.Errorf("manual configuration requires --runtime-version"))
		}
	default:
		// Autodetection via Cargo manifest.
		fmt.Printf("Attempting to autodetect runtime metadata from '%s'...\n", cargoTomlName)

		data, err := os.ReadFile(cargoTomlName)
		if err != nil {
			cobra.CheckErr(fmt.Errorf("failed to read Cargo manifest: %w", err))
		}

		err = toml.Unmarshal(data, &cm)
		if err != nil {
			cobra.CheckErr(fmt.Errorf("malformed Cargo manifest: %w", err))
		}
	}

	var manifest bundle.Manifest
	switch overrideRuntimeName {
	case "":
		// Automatic name determination based on the cargo manifest.
		manifest.Name = cm.Package.Name
	default:
		// Manually configured runtime name.
		manifest.Name = overrideRuntimeName
	}

	fmt.Printf("Using runtime name: %s\n", manifest.Name)

	var versionStr string
	switch overrideRuntimeVersion {
	case "":
		// Automatic version determination based on the cargo manifest.
		versionStr = cm.Package.Version
	default:
		// Manually configured runtime version.
		versionStr = overrideRuntimeVersion
	}

	var err error
	manifest.Version, err = version.FromString(versionStr)
	if err != nil {
		cobra.CheckErr(fmt.Errorf("malformed runtime version: %w", err))
	}

	fmt.Printf("Using runtime version: %s\n", manifest.Version)

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

	return &manifest
}

func getComponentID() (id component.ID) {
	err := id.UnmarshalText([]byte(componentId))
	cobra.CheckErr(err)
	return
}

func addComponent(bnd *bundle.Bundle, id component.ID, elfExecutableFn string) {
	rawCompId, _ := id.MarshalText()
	execName := fmt.Sprintf(execNameFmt, string(rawCompId))

	comp := bundle.Component{
		Kind:       id.Kind,
		Name:       id.Name,
		Executable: execName,
	}

	// Collect all assets for the bundle.
	type runtimeFile struct {
		fn, descr, dst string
	}
	wantFiles := []runtimeFile{
		{
			fn:    elfExecutableFn,
			descr: fmt.Sprintf("%s: ELF binary", string(rawCompId)),
			dst:   execName,
		},
	}
	if sgxExecutableFn != "" {
		sgxExecName := fmt.Sprintf(sgxExecNameFmt, string(rawCompId))
		wantFiles = append(wantFiles, runtimeFile{
			fn:    sgxExecutableFn,
			descr: fmt.Sprintf("%s: SGX binary", string(rawCompId)),
			dst:   sgxExecName,
		})
		comp.SGX = &bundle.SGXMetadata{
			Executable: sgxExecName,
		}

		if sgxSignatureFn != "" {
			sgxSigName := fmt.Sprintf(sgxSigNameFmt, string(rawCompId))
			wantFiles = append(wantFiles, runtimeFile{
				fn:    sgxSignatureFn,
				descr: fmt.Sprintf("%s: SGX signature", string(rawCompId)),
				dst:   sgxSigName,
			})
			comp.SGX.Signature = sgxSigName
		}
	}

	bnd.Manifest.Components = append(bnd.Manifest.Components, &comp)

	err := bnd.Manifest.Validate()
	if err != nil {
		cobra.CheckErr(fmt.Errorf("failed to validate manifest: %w", err))
	}

	for _, v := range wantFiles {
		if v.fn == "" {
			cobra.CheckErr(fmt.Errorf("missing runtime asset '%s'", v.descr))
		}
		_ = bnd.Add(v.dst, bundle.NewFileData(v.fn))
	}

	bnd.ResetManifest()
}

func writeBundle(bnd *bundle.Bundle) {
	// Write the bundle out.
	outFn := fmt.Sprintf("%s.orc", bnd.Manifest.Name)
	if bundleFn != "" {
		outFn = bundleFn
	}
	if err := bnd.Write(outFn); err != nil {
		cobra.CheckErr(fmt.Errorf("failed to write output bundle: %w", err))
	}
}

func showComponent(bnd *bundle.Bundle, comp *bundle.Component, legacy bool) {
	fmt.Printf("- %s", comp.ID())
	if legacy {
		fmt.Printf(" [legacy]")
	}
	fmt.Println()
	indent := "  "

	if comp.Executable != "" {
		fmt.Printf("%sExecutable:     %s\n", indent, comp.Executable)
	}

	fmt.Printf("%sTEE kind:       %s\n", indent, comp.TEEKind())

	switch {
	case comp.SGX != nil:
		showSgxComponent(indent, bnd, comp)
	case comp.TDX != nil:
		showTdxComponent(indent, bnd, comp)
	default:
	}
}

func showSgxComponent(indent string, bnd *bundle.Bundle, comp *bundle.Component) {
	if comp.SGX == nil {
		return
	}

	fmt.Printf("%sSGXS:           %s\n", indent, comp.SGX.Executable)

	mrEnclave, err := bnd.MrEnclave(comp.ID())
	if err != nil {
		cobra.CheckErr(fmt.Errorf("failed to compute MRENCLAVE: %w", err))
	}
	fmt.Printf("%sSGXS MRENCLAVE: %s\n", indent, mrEnclave)

	if comp.SGX.Signature != "" {
		fmt.Printf("%sSGXS signature: %s\n", indent, comp.SGX.Signature)

		sigData, err := bundle.ReadAllData(bnd.Data[comp.SGX.Signature])
		cobra.CheckErr(err)
		sigPk, sigStruct, err := sigstruct.Verify(sigData)
		cobra.CheckErr(err) // Already checked during Open so it should never fail.

		var mrSigner sgx.MrSigner
		err = mrSigner.FromPublicKey(sigPk)
		cobra.CheckErr(err)

		fmt.Printf("%sSGXS SIGSTRUCT:\n", indent)
		fmt.Printf("%s  Build date:       %s\n", indent, sigStruct.BuildDate)
		fmt.Printf("%s  MiscSelect:       %08X\n", indent, sigStruct.MiscSelect)
		fmt.Printf("%s  MiscSelect mask:  %08X\n", indent, sigStruct.MiscSelectMask)
		fmt.Printf("%s  Attributes flags: %016X\n", indent, sigStruct.Attributes.Flags)

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
				fmt.Printf("%s    - %s\n", indent, fm.name)
			}
		}

		fmt.Printf("%s  Attributes XFRM:  %016X\n", indent, sigStruct.Attributes.Xfrm)
		fmt.Printf("%s  Attributes mask:  %016X %016X\n", indent, sigStruct.AttributesMask[0], sigStruct.AttributesMask[1])
		fmt.Printf("%s  MRENCLAVE:        %s\n", indent, sigStruct.EnclaveHash)
		fmt.Printf("%s  MRSIGNER:         %s\n", indent, mrSigner)
		fmt.Printf("%s  ISV product ID:   %d\n", indent, sigStruct.ISVProdID)
		fmt.Printf("%s  ISV SVN:          %d\n", indent, sigStruct.ISVSVN)
	} else {
		fmt.Printf("%sSGXS signature: [UNSIGNED]\n", indent)
	}
}

func showTdxComponent(indent string, bnd *bundle.Bundle, comp *bundle.Component) {
	if comp.TDX == nil {
		return
	}

	fmt.Printf("%sFirmware:       %s\n", indent, comp.TDX.Firmware)
	if comp.TDX.HasKernel() {
		fmt.Printf("%sKernel:         %s\n", indent, comp.TDX.Kernel)

		if comp.TDX.HasInitRD() {
			fmt.Printf("%sInitRD:         %s\n", indent, comp.TDX.InitRD)
		}

		if len(comp.TDX.ExtraKernelOptions) > 0 {
			fmt.Printf("%sExtra kernel options:\n", indent)
			for _, v := range comp.TDX.ExtraKernelOptions {
				fmt.Printf("%s  %s\n", indent, v)
			}
		}
	}

	if comp.TDX.HasStage2() {
		fmt.Printf("%sStage 2:        %s\n", indent, comp.TDX.Stage2Image)
	}

	fmt.Printf("%sResources:\n", indent)
	fmt.Printf("%s  CPUs:    %d\n", indent, comp.TDX.Resources.CPUCount)
	fmt.Printf("%s  Memory:  %d MiB\n", indent, comp.TDX.Resources.Memory)
}

// sgxGenerateKey generates a 3072-bit RSA key with public exponent 3 as required for SGX.
//
// The code below is adopted from the Go standard library as it is otherwise not possible to
// customize the exponent.
func sgxGenerateKey(random io.Reader) (*rsa.PrivateKey, error) {
	priv := new(rsa.PrivateKey)
	priv.E = 3
	bits := 3072
	nprimes := 2

	bigOne := big.NewInt(1)
	primes := make([]*big.Int, nprimes)

NextSetOfPrimes:
	for {
		todo := bits
		// crypto/rand should set the top two bits in each prime.
		// Thus each prime has the form
		//   p_i = 2^bitlen(p_i) × 0.11... (in base 2).
		// And the product is:
		//   P = 2^todo × α
		// where α is the product of nprimes numbers of the form 0.11...
		//
		// If α < 1/2 (which can happen for nprimes > 2), we need to
		// shift todo to compensate for lost bits: the mean value of 0.11...
		// is 7/8, so todo + shift - nprimes * log2(7/8) ~= bits - 1/2
		// will give good results.
		if nprimes >= 7 {
			todo += (nprimes - 2) / 5
		}
		for i := 0; i < nprimes; i++ {
			var err error
			primes[i], err = rand.Prime(random, todo/(nprimes-i))
			if err != nil {
				return nil, err
			}
			todo -= primes[i].BitLen()
		}

		// Make sure that primes is pairwise unequal.
		for i, prime := range primes {
			for j := 0; j < i; j++ {
				if prime.Cmp(primes[j]) == 0 {
					continue NextSetOfPrimes
				}
			}
		}

		n := new(big.Int).Set(bigOne)
		totient := new(big.Int).Set(bigOne)
		pminus1 := new(big.Int)
		for _, prime := range primes {
			n.Mul(n, prime)
			pminus1.Sub(prime, bigOne)
			totient.Mul(totient, pminus1)
		}
		if n.BitLen() != bits {
			// This should never happen for nprimes == 2 because
			// crypto/rand should set the top two bits in each prime.
			// For nprimes > 2 we hope it does not happen often.
			continue NextSetOfPrimes
		}

		priv.D = new(big.Int)
		e := big.NewInt(int64(priv.E))
		ok := priv.D.ModInverse(e, totient)

		if ok != nil {
			priv.Primes = primes
			priv.N = n
			break
		}
	}

	priv.Precompute()
	return priv, nil
}

func main() {
	_ = rootCmd.Execute()
}

func init() {
	// SGX flags.
	sgxFlags := flag.NewFlagSet("", flag.ContinueOnError)
	sgxFlags.StringVar(&sgxExecutableFn, "sgx-executable", "", "SGXS executable for runtimes with TEE support")
	sgxFlags.StringVar(&sgxSignatureFn, "sgx-signature", "", "detached SGXS signature for runtimes with TEE support")
	compAddCmd.Flags().AddFlagSet(sgxFlags)

	// Init cmd.
	initFlags := flag.NewFlagSet("", flag.ContinueOnError)
	initFlags.BoolVar(&noAutodetection, "custom", false, "disable autodetection")
	initFlags.StringVar(&bundleFn, "output", "", "output bundle filename")
	initFlags.StringVar(&overrideRuntimeName, "runtime-name", "", "override runtime name")
	initFlags.StringVar(&overrideRuntimeVersion, "runtime-version", "", "override runtime version")
	initFlags.StringVar(&overrideRuntimeID, "runtime-id", "", "override runtime ID")
	initCmd.Flags().AddFlagSet(initFlags)
	initCmd.Flags().AddFlagSet(sgxFlags)

	// Component flags.
	compFlags := flag.NewFlagSet("", flag.ContinueOnError)
	compFlags.StringVar(&componentId, "component", "ronl", "component kind.name (default: ronl)")

	// SGX signing cmds.
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
	sgxGetSignDataCmd.Flags().AddFlagSet(compFlags)
	sgxSetSigCmd.Flags().AddFlagSet(signFlags)
	sgxSetSigCmd.Flags().AddFlagSet(compFlags)

	rootCmd.AddCommand(initCmd)
	rootCmd.AddCommand(compAddCmd)
	rootCmd.AddCommand(sgxGetSignDataCmd)
	rootCmd.AddCommand(sgxSetSigCmd)
	rootCmd.AddCommand(showCmd)
}
