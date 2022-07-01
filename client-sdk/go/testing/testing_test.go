package testing

import (
	"encoding/hex"
	"fmt"
	"testing"
)

func TestPrintTestKeys(t *testing.T) {
	fmt.Printf("A: %v\n", Alice.Signer.Public().String())
	fmt.Printf("B: %v\n", Bob.Signer.Public().String())
	fmt.Printf("C: %v\n", Charlie.Signer.Public().String())
	fmt.Printf("D: %v\n", Dave.Signer.Public().String())
	fmt.Printf("D(ETH): %v\n", hex.EncodeToString(Dave.EthAddress[:]))
	fmt.Printf("E: %v\n", Erin.Signer.Public().String())
	fmt.Printf("E(ETH): %v\n", hex.EncodeToString(Erin.EthAddress[:]))
	fmt.Printf("F: %v\n", Frank.Signer.Public().String())
	fmt.Printf("G: %v\n", Grace.Signer.Public().String())
}
