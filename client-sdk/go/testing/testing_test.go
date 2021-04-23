package testing

import (
	"fmt"
	"testing"
)

func TestPrintTestKeys(t *testing.T) {
	fmt.Printf("A: %v\n", Alice.Signer.Public().String())
	fmt.Printf("B: %v\n", Bob.Signer.Public().String())
	fmt.Printf("C: %v\n", Charlie.Signer.Public().String())
	fmt.Printf("D: %v\n", Dave.Signer.Public().String())
}
