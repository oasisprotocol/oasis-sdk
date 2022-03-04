package main

import (
	"fmt"

	"github.com/oasisprotocol/oasis-core/go/common/sgx/sigstruct"
	"github.com/oasisprotocol/oasis-core/go/runtime/bundle"
)

// sgxVerifySignature verifies that the SGXS signature in the bundle is well-formed and actually
// matches the SGXS.
//
// TODO: Consider moving this to bundle validation.
func sgxVerifySignature(bnd *bundle.Bundle) error {
	if bnd.Manifest.SGX == nil {
		return fmt.Errorf("no SGX runtimes in manifest")
	}
	if bnd.Manifest.SGX.Signature == "" {
		return fmt.Errorf("no SGXS signature in manifest")
	}

	mrEnclave, err := bnd.MrEnclave()
	if err != nil {
		return fmt.Errorf("failed to derive MRENCLAVE: %w", err)
	}
	_, sigStruct, err := sigstruct.Verify(bnd.Data[bnd.Manifest.SGX.Signature])
	if err != nil {
		return fmt.Errorf("failed to verify sigstruct: %w", err)
	}

	if sigStruct.EnclaveHash != *mrEnclave {
		return fmt.Errorf("sigstruct does not match SGXS (got: %s expected: %s)", sigStruct.EnclaveHash, *mrEnclave)
	}

	return nil
}
