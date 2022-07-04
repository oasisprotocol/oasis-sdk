package common

import (
	"encoding/json"
	"fmt"
	"strings"
	"unicode/utf8"

	"github.com/spf13/cobra"

	"github.com/oasisprotocol/oasis-core/go/common/cbor"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/modules/contracts"
)

// PrettyJSONMarshal returns pretty-printed JSON encoding of v.
func PrettyJSONMarshal(v interface{}) ([]byte, error) {
	formatted, err := json.MarshalIndent(v, "", "  ")
	if err != nil {
		return nil, fmt.Errorf("failed to marshal to pretty JSON: %w", err)
	}
	return formatted, nil
}

// JSONMarshalKey encodes k as UTF-8 string if valid, or Base64 otherwise.
func JSONMarshalKey(k interface{}) (keyJSON []byte, err error) {
	keyBytes, ok := k.([]byte)
	if ok && utf8.Valid(keyBytes) {
		// Marshal valid UTF-8 string.
		keyJSON, err = json.Marshal(string(keyBytes))
	} else {
		// Marshal string or Base64 otherwise.
		keyJSON, err = json.Marshal(k)
	}
	return
}

// JSONPrintKeyValueTuple traverses potentially large number of items and prints JSON representation
// of them.
//
// Marshalling is done externally without holding resulting JSON string in-memory.
// Cbor decoding of each value is tried first. If it fails, the binary content is preserved.
// Universal marshalling of map[interface{}]interface{} types is also supported.
// Each key is encoded as string if it contains valid UTF-8 value. Otherwise, Base64 is used.
func JSONPrintKeyValueTuple(items []contracts.InstanceStorageKeyValue) {
	first := true
	fmt.Printf("{")
	for _, kv := range items {
		if !first {
			fmt.Printf(",")
		}
		first = false
		var val interface{}
		err := cbor.Unmarshal(kv.Value, &val)
		if err != nil {
			// Value is not CBOR, use raw value instead.
			val = kv.Value
		}
		keyJSON, err := JSONMarshalKey(kv.Key)
		cobra.CheckErr(err)

		valJSON := JSONMarshalUniversalValue(val)
		fmt.Printf("%s:%s", keyJSON, valJSON)
	}
	fmt.Printf("}\n")
}

// JSONMarshalUniversalValue is a wrapper for the built-in JSON encoder which adds support for
// marshalling map[interface{}]interface{}.
//
// Each key is encoded as string if it contains valid UTF-8 value. Otherwise, Base64 is used.
func JSONMarshalUniversalValue(v interface{}) []byte {
	// Try array.
	if valTest, ok := v.([]interface{}); ok {
		e := make([]string, 0, len(valTest))
		for _, v := range valTest {
			valJSON := JSONMarshalUniversalValue(v)
			e = append(e, string(valJSON))
		}
		return []byte(fmt.Sprintf("[%s]", strings.Join(e, ",")))
	}

	// Try universal map.
	if valTest, ok := v.(map[interface{}]interface{}); ok {
		e := make([]string, 0, len(valTest))
		for k, v := range valTest {
			keyJSON, err := JSONMarshalKey(k)
			cobra.CheckErr(err)

			valJSON := JSONMarshalUniversalValue(v)

			e = append(e, fmt.Sprintf("%s:%s", keyJSON, valJSON))
		}
		return []byte(fmt.Sprintf("{%s}", strings.Join(e, ",")))
	}

	// Primitive type - use built-in JSON encoder.
	vJSON, err := json.Marshal(v)
	cobra.CheckErr(err)
	return vJSON
}
