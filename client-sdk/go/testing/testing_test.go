package testing

import (
	"encoding/hex"
	"fmt"
	"testing"
)

func TestPrintTestKeys(t *testing.T) {
	fmt.Printf("A: %v\n", Alice.Signer.Public().String())
	fmt.Printf("A addr: %s\n", Alice.Address)
	fmt.Printf("B: %v\n", Bob.Signer.Public().String())
	fmt.Printf("B addr: %s\n", Bob.Address)
	fmt.Printf("C: %v\n", Charlie.Signer.Public().String())
	fmt.Printf("D: %v\n", Dave.Signer.Public().String())
	fmt.Printf("D(ETH): %v\n", hex.EncodeToString(Dave.EthAddress[:]))
}
