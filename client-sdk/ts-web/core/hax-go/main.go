package main

import (
	"fmt"
	"reflect"
	"strings"
	"time"

	"github.com/oasisprotocol/oasis-core/go/common/quantity"
	genesis "github.com/oasisprotocol/oasis-core/go/genesis/api"
)

type usedType struct {
	ref string
	source string
}

var used = []*usedType{}
var memo = map[reflect.Type]*usedType{}

var prefixByPackage = map[string]string{
	"github.com/oasisprotocol/oasis-core/go/beacon/api": "Beacon",
	"github.com/oasisprotocol/oasis-core/go/common/cbor": "CBOR",
	"github.com/oasisprotocol/oasis-core/go/common/crypto/signature": "Signature",
	"github.com/oasisprotocol/oasis-core/go/common/entity": "Entity",
	"github.com/oasisprotocol/oasis-core/go/common/node": "Node",
	"github.com/oasisprotocol/oasis-core/go/common/sgx": "SGX",
	"github.com/oasisprotocol/oasis-core/go/common/version": "Version",
	"github.com/oasisprotocol/oasis-core/go/consensus/genesis": "Consensus",
	"github.com/oasisprotocol/oasis-core/go/genesis/api": "Genesis",
	"github.com/oasisprotocol/oasis-core/go/governance/api": "Governance",
	"github.com/oasisprotocol/oasis-core/go/keymanager/api": "KeyManager",
	"github.com/oasisprotocol/oasis-core/go/registry/api": "Registry",
	"github.com/oasisprotocol/oasis-core/go/roothash/api": "Roothash",
	"github.com/oasisprotocol/oasis-core/go/scheduler/api": "Scheduler",
	"github.com/oasisprotocol/oasis-core/go/staking/api": "Staking",
	"github.com/oasisprotocol/oasis-core/go/storage/mkvs/writelog": "Storage",
	"github.com/oasisprotocol/oasis-core/go/upgrade/api": "Upgrade",
}
var prefixConsulted = map[string]bool{}

func visitType(t reflect.Type) string {
	switch t {
	case reflect.TypeOf(time.Time{}):
		t = reflect.TypeOf(int64(0))
	case reflect.TypeOf(quantity.Quantity{}):
		t = reflect.TypeOf([]byte{})
	}
	if ut, ok := memo[t]; ok {
		return ut.ref
	}
	switch t.Kind() {
	// Invalid begone
	case reflect.Bool:
		return "boolean"
	case reflect.Int, reflect.Int8, reflect.Int16, reflect.Int32, reflect.Uint, reflect.Uint8, reflect.Uint16, reflect.Uint32:
		return "number"
	case reflect.Int64, reflect.Uint64, reflect.Uintptr:
		return "longnum"
	case reflect.Float32, reflect.Float64:
		return "number"
	// Complex64, Complex128 begone
	case reflect.Array, reflect.Slice:
		if t.Elem().Kind() == reflect.Uint8 {
			return "Uint8Array"
		}
		return fmt.Sprintf("%s[]", visitType(t.Elem()))
	// Chan begone
	// Func begone
	// Interface begone
	case reflect.Map:
		if t.Key().Kind() == reflect.String {
			return fmt.Sprintf("{[key: string]: %s}", visitType(t.Elem()))
		}
		return fmt.Sprintf("Map<%s, %s>", visitType(t.Key()), visitType(t.Elem()))
	case reflect.Ptr:
		return visitType(t.Elem())
	case reflect.String:
		return "string"
	case reflect.Struct:
		prefixConsulted[t.PkgPath()] = true
		prefix, ok := prefixByPackage[t.PkgPath()]
		if !ok {
			panic(fmt.Sprintf("unset package prefix %s", t.PkgPath()))
		}
		var ref string
		if prefix == t.Name() {
			ref = t.Name()
		} else {
			ref = prefix + t.Name()
		}
		extends := ""
		sourceFields := ""
		mode := "object"
		for i := 0; i < t.NumField(); i++ {
			f := t.Field(i)
			if f.Anonymous {
				if extends == "" {
					extends = fmt.Sprintf(" extends %s", visitType(f.Type))
				} else {
					panic("multiple embedded types")
				}
				continue
			}
			var name string
			var optional string
			if cborTag, ok := f.Tag.Lookup("cbor"); ok {
				parts := strings.Split(cborTag, ",")
				name = parts[0]
				parts = parts[1:]
				if name == "" {
					for _, part := range parts {
						switch part {
						case "toarray":
							if sourceFields != "" {
								panic("changing struct mode after fields are rendered")
							}
							mode = "array"
						default:
							panic(fmt.Sprintf("unhandled json tag %s", part))
						}
					}
					continue
				}
				for _, part := range parts {
					panic(fmt.Sprintf("unhandled cbor tag %s", part))
				}
			} else if jsonTag, ok := f.Tag.Lookup("json"); ok {
				parts := strings.Split(jsonTag, ",")
				name = parts[0]
				parts = parts[1:]
				for _, part := range parts {
					switch part {
					case "omitempty":
						optional = "?"
					default:
						panic(fmt.Sprintf("unhandled json tag %s", part))
					}
				}
			} else {
				name = f.Name
			}
			switch mode {
			case "object":
				sourceFields += fmt.Sprintf("    %s%s: %s;\n", name, optional, visitType(f.Type))
			case "array":
				if optional != "" {
					panic("unhandled optional in mode array")
				}
				sourceFields += fmt.Sprintf("    %s: %s,\n", name, visitType(f.Type))
			default:
				panic(fmt.Sprintf("unhandled struct field in mode %s", mode))
			}
		}
		if sourceFields == "" && extends != "" {
			return extends[9:] // todo: less hacky bookkeeping
		}
		if mode == "object" && sourceFields == "" && extends == "" {
			mode = "empty-map"
		}
		var source string
		switch mode {
		case "object":
			source = fmt.Sprintf("export interface %s%s {\n%s}\n", ref, extends, sourceFields)
		case "array":
			if extends != "" {
				panic("unhandled extends in mode array")
			}
			source = fmt.Sprintf("export type %s = [\n%s];\n", ref, sourceFields)
		case "empty-map":
			if extends != "" {
				panic("unhandled extends in mode empty-map")
			}
			if sourceFields != "" {
				panic("unhandled source fields in mode empty-map")
			}
			source = fmt.Sprintf("export type %s = Map<never, never>;\n", ref)
		}
		ut := usedType{ref, source}
		used = append(used, &ut)
		memo[t] = &ut
		return ref
	// UnsafePointer begone
	default:
		panic(fmt.Sprintf("unhandled kind %v", t.Kind()))
	}
}

func write() {
	// todo: sort
	for _, ut := range used {
		fmt.Print(ut.source)
	}
}

func main() {
	visitType(reflect.TypeOf((*genesis.Document)(nil)).Elem())
	write()
	for prefix, _ := range prefixByPackage {
		if !prefixConsulted[prefix] {
			panic(fmt.Sprintf("unused prefix %s", prefix))
		}
	}
}
