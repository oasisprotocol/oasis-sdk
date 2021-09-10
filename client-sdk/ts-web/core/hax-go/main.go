package main

import (
	"context"
	"fmt"
	"io"
	"os"
	"reflect"
	"sort"
	"strings"
	"time"

	beacon "github.com/oasisprotocol/oasis-core/go/beacon/api"
	"github.com/oasisprotocol/oasis-core/go/common/cbor"
	"github.com/oasisprotocol/oasis-core/go/common/crypto/pvss"
	"github.com/oasisprotocol/oasis-core/go/common/crypto/signature"
	"github.com/oasisprotocol/oasis-core/go/common/entity"
	"github.com/oasisprotocol/oasis-core/go/common/node"
	"github.com/oasisprotocol/oasis-core/go/common/pubsub"
	"github.com/oasisprotocol/oasis-core/go/common/quantity"
	consensus "github.com/oasisprotocol/oasis-core/go/consensus/api"
	"github.com/oasisprotocol/oasis-core/go/consensus/api/transaction"
	control "github.com/oasisprotocol/oasis-core/go/control/api"
	governance "github.com/oasisprotocol/oasis-core/go/governance/api"
	keymanager "github.com/oasisprotocol/oasis-core/go/keymanager/api"
	registry "github.com/oasisprotocol/oasis-core/go/registry/api"
	roothash "github.com/oasisprotocol/oasis-core/go/roothash/api"
	runtimeClient "github.com/oasisprotocol/oasis-core/go/runtime/client/api"
	enclaverpc "github.com/oasisprotocol/oasis-core/go/runtime/enclaverpc/api"
	scheduler "github.com/oasisprotocol/oasis-core/go/scheduler/api"
	staking "github.com/oasisprotocol/oasis-core/go/staking/api"
	storage "github.com/oasisprotocol/oasis-core/go/storage/api"
	workerStorage "github.com/oasisprotocol/oasis-core/go/worker/storage/api"
)

type usedType struct {
	ref string
	source string
}

var used = []*usedType{}
var memo = map[reflect.Type]*usedType{}

var customStructNames = map[reflect.Type]string{
	reflect.TypeOf(consensus.Parameters{}): "ConsensusLightParameters",
}
var customStructNamesConsulted = map[reflect.Type]bool{}

var prefixByPackage = map[string]string{
	"net": "Net",

	"github.com/oasisprotocol/oasis-core/go/beacon/api": "Beacon",
	"github.com/oasisprotocol/oasis-core/go/common/cbor": "CBOR",
	"github.com/oasisprotocol/oasis-core/go/common/crypto/pvss": "PVSS",
	"github.com/oasisprotocol/oasis-core/go/common/crypto/signature": "Signature",
	"github.com/oasisprotocol/oasis-core/go/common/entity": "Entity",
	"github.com/oasisprotocol/oasis-core/go/common/node": "Node",
	"github.com/oasisprotocol/oasis-core/go/common/sgx": "SGX",
	"github.com/oasisprotocol/oasis-core/go/common/version": "Version",
	"github.com/oasisprotocol/oasis-core/go/consensus/api": "Consensus",
	"github.com/oasisprotocol/oasis-core/go/consensus/api/transaction": "Consensus",
	"github.com/oasisprotocol/oasis-core/go/consensus/api/transaction/results": "Consensus",
	"github.com/oasisprotocol/oasis-core/go/consensus/genesis": "Consensus",
	"github.com/oasisprotocol/oasis-core/go/control/api": "Control",
	"github.com/oasisprotocol/oasis-core/go/genesis/api": "Genesis",
	"github.com/oasisprotocol/oasis-core/go/governance/api": "Governance",
	"github.com/oasisprotocol/oasis-core/go/keymanager/api": "KeyManager",
	"github.com/oasisprotocol/oasis-core/go/registry/api": "Registry",
	"github.com/oasisprotocol/oasis-core/go/roothash/api": "RootHash",
	"github.com/oasisprotocol/oasis-core/go/roothash/api/block": "RootHash",
	"github.com/oasisprotocol/oasis-core/go/roothash/api/commitment": "RootHash",
	"github.com/oasisprotocol/oasis-core/go/runtime/client/api": "RuntimeClient",
	"github.com/oasisprotocol/oasis-core/go/runtime/enclaverpc/api": "EnclaveRPC",
	"github.com/oasisprotocol/oasis-core/go/scheduler/api": "Scheduler",
	"github.com/oasisprotocol/oasis-core/go/staking/api": "Staking",
	"github.com/oasisprotocol/oasis-core/go/storage/api": "Storage",
	"github.com/oasisprotocol/oasis-core/go/storage/mkvs/checkpoint": "Storage",
	"github.com/oasisprotocol/oasis-core/go/storage/mkvs/node": "Storage",
	"github.com/oasisprotocol/oasis-core/go/storage/mkvs/syncer": "Storage",
	"github.com/oasisprotocol/oasis-core/go/storage/mkvs/writelog": "Storage",
	"github.com/oasisprotocol/oasis-core/go/upgrade/api": "Upgrade",
	"github.com/oasisprotocol/oasis-core/go/worker/common/api": "WorkerCommon",
	"github.com/oasisprotocol/oasis-core/go/worker/storage/api": "WorkerStorage",
}
var prefixConsulted = map[string]bool{}

func getStructName(t reflect.Type) string {
	if ref, ok := customStructNames[t]; ok {
		customStructNamesConsulted[t] = true
		return ref
	}
	prefixConsulted[t.PkgPath()] = true
	prefix, ok := prefixByPackage[t.PkgPath()]
	if !ok {
		panic(fmt.Sprintf("unset package prefix %s", t.PkgPath()))
	}
	if prefix == t.Name() {
		return t.Name()
	}
	return prefix + t.Name()
}

var mapKeyNames = map[reflect.Type]string {}
var mapKeyNamesConsulted = map[reflect.Type]bool{}

func getMapKeyName(t reflect.Type) string {
	switch t.Key() {
	case reflect.TypeOf(transaction.Op("")):
		return "op"
	case reflect.TypeOf(staking.StakeClaim("")):
		return "claim"
	}
	return "key"
}

func visitType(t reflect.Type) string {
	_, _ = fmt.Fprintf(os.Stderr, "visiting type %v\n", t)
	switch t {
	case reflect.TypeOf(time.Time{}):
		t = reflect.TypeOf(int64(0))
	case reflect.TypeOf(quantity.Quantity{}):
		t = reflect.TypeOf([]byte{})
	case reflect.TypeOf(pvss.Point{}):
		t = reflect.TypeOf([]byte{})
	case reflect.TypeOf(pvss.Scalar{}):
		t = reflect.TypeOf([]byte{})
	case reflect.TypeOf((*io.Writer)(nil)).Elem():
		t = reflect.TypeOf([]byte{})
	case reflect.TypeOf((*storage.WriteLogIterator)(nil)).Elem():
		t = reflect.TypeOf(storage.SyncChunk{})
	case reflect.TypeOf((*signature.Signed)(nil)).Elem():
		_, _ = fmt.Fprintf(os.Stderr, "signed %v\n", t) // %%%
	case reflect.TypeOf(cbor.RawMessage{}):
		return "unknown"
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
			return fmt.Sprintf("{[%s: string]: %s}", getMapKeyName(t), visitType(t.Elem()))
		}
		return fmt.Sprintf("Map<%s, %s>", visitType(t.Key()), visitType(t.Elem()))
	case reflect.Ptr:
		return visitType(t.Elem())
	case reflect.String:
		return "string"
	case reflect.Struct:
		ref := getStructName(t)
		extends := ""
		sourceFields := ""
		mode := "object"
		for i := 0; i < t.NumField(); i++ {
			f := t.Field(i)
			_, _ = fmt.Fprintf(os.Stderr, "visiting field %v\n", f)
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
				if name == "-" {
					continue
				}
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
			if f.PkgPath != "" {
				// skip private fields
				continue
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

var skipMethods = map[string]bool{
	"github.com/oasisprotocol/oasis-core/go/roothash/api.Backend.TrackRuntime": true,
	"github.com/oasisprotocol/oasis-core/go/storage/api.Backend.Initialized": true,
	"github.com/oasisprotocol/oasis-core/go/consensus/api.ClientBackend.Beacon": true,
	"github.com/oasisprotocol/oasis-core/go/consensus/api.ClientBackend.Registry": true,
	"github.com/oasisprotocol/oasis-core/go/consensus/api.ClientBackend.Staking": true,
	"github.com/oasisprotocol/oasis-core/go/consensus/api.ClientBackend.Scheduler": true,
	"github.com/oasisprotocol/oasis-core/go/consensus/api.ClientBackend.Governance": true,
	"github.com/oasisprotocol/oasis-core/go/consensus/api.ClientBackend.RootHash": true,
	"github.com/oasisprotocol/oasis-core/go/consensus/api.ClientBackend.State": true,
}
var skipMethodsConsulted = map[string]bool{}

func visitClient(t reflect.Type) {
	_, _ = fmt.Fprintf(os.Stderr, "visiting client %v\n", t)
	for i := 0; i < t.NumMethod(); i++ {
		m := t.Method(i)
		_, _ = fmt.Fprintf(os.Stderr, "visiting method %v\n", m)
		sig := fmt.Sprintf("%s.%s.%s", t.PkgPath(), t.Name(), m.Name)
		if skipMethods[sig] {
			skipMethodsConsulted[sig] = true
			continue
		}
		for j := 0; j < m.Type.NumIn(); j++ {
			u := m.Type.In(j)
			// skip context
			if u == reflect.TypeOf((*context.Context)(nil)).Elem() {
				continue
			}
			visitType(u)
		}
		for j := 0; j < m.Type.NumOut(); j++ {
			u := m.Type.Out(j)
			// skip subscription
			if u == reflect.TypeOf((*pubsub.Subscription)(nil)) {
				continue
			}
			if u == reflect.TypeOf((*pubsub.ClosableSubscription)(nil)).Elem() {
				continue
			}
			// skip error
			if u == reflect.TypeOf((*error)(nil)).Elem() {
				continue
			}
			// visit stream datum instead
			if u.Kind() == reflect.Chan {
				visitType(u.Elem())
				continue
			}
			visitType(u)
		}
	}
}

func write() {
	sort.Slice(used, func(i, j int) bool {
		return strings.ToLower(used[i].ref) < strings.ToLower(used[j].ref)
	})
	for _, ut := range used {
		fmt.Print(ut.source)
	}
}

func main() {
	visitClient(reflect.TypeOf((*beacon.PVSSBackend)(nil)).Elem())
	visitType(reflect.TypeOf((*beacon.PVSSCommit)(nil)).Elem())
	visitType(reflect.TypeOf((*beacon.PVSSReveal)(nil)).Elem())
	visitType(reflect.TypeOf((*beacon.EpochTime)(nil)).Elem())

	visitClient(reflect.TypeOf((*scheduler.Backend)(nil)).Elem())

	visitClient(reflect.TypeOf((*registry.Backend)(nil)).Elem())
	visitType(reflect.TypeOf((*entity.SignedEntity)(nil)).Elem())
	visitType(reflect.TypeOf((*node.MultiSignedNode)(nil)).Elem())
	visitType(reflect.TypeOf((*registry.UnfreezeNode)(nil)).Elem())
	visitType(reflect.TypeOf((*registry.Runtime)(nil)).Elem())

	visitClient(reflect.TypeOf((*staking.Backend)(nil)).Elem())
	visitType(reflect.TypeOf((*staking.Transfer)(nil)).Elem())
	visitType(reflect.TypeOf((*staking.Burn)(nil)).Elem())
	visitType(reflect.TypeOf((*staking.Escrow)(nil)).Elem())
	visitType(reflect.TypeOf((*staking.ReclaimEscrow)(nil)).Elem())
	visitType(reflect.TypeOf((*staking.AmendCommissionSchedule)(nil)).Elem())
	visitType(reflect.TypeOf((*staking.Allow)(nil)).Elem())
	visitType(reflect.TypeOf((*staking.Withdraw)(nil)).Elem())

	visitClient(reflect.TypeOf((*keymanager.Backend)(nil)).Elem())
	visitType(reflect.TypeOf((*keymanager.SignedPolicySGX)(nil)).Elem())

	visitClient(reflect.TypeOf((*roothash.Backend)(nil)).Elem())
	visitType(reflect.TypeOf((*roothash.ExecutorCommit)(nil)).Elem())
	visitType(reflect.TypeOf((*roothash.ExecutorProposerTimeoutRequest)(nil)).Elem())
	visitType(reflect.TypeOf((*roothash.Evidence)(nil)).Elem())

	visitClient(reflect.TypeOf((*governance.Backend)(nil)).Elem())
	visitType(reflect.TypeOf((*governance.ProposalContent)(nil)).Elem())
	visitType(reflect.TypeOf((*governance.ProposalVote)(nil)).Elem())

	visitClient(reflect.TypeOf((*runtimeClient.RuntimeClient)(nil)).Elem())
	visitClient(reflect.TypeOf((*enclaverpc.Transport)(nil)).Elem())
	visitClient(reflect.TypeOf((*storage.Backend)(nil)).Elem())
	visitClient(reflect.TypeOf((*workerStorage.StorageWorker)(nil)).Elem())
	visitClient(reflect.TypeOf((*consensus.ClientBackend)(nil)).Elem())
	visitClient(reflect.TypeOf((*control.NodeController)(nil)).Elem())
	visitClient(reflect.TypeOf((*control.DebugController)(nil)).Elem())

	write()
	for t := range customStructNames {
		if !customStructNamesConsulted[t] {
			panic(fmt.Sprintf("unused custom type name %v", t))
		}
	}
	for prefix := range prefixByPackage {
		if !prefixConsulted[prefix] {
			panic(fmt.Sprintf("unused prefix %s", prefix))
		}
	}
	for t := range mapKeyNames {
		if !mapKeyNamesConsulted[t] {
			panic(fmt.Sprintf("unused map key name %v", t))
		}
	}
	for sig := range skipMethods {
		if !skipMethodsConsulted[sig] {
			panic(fmt.Sprintf("unused skip method %s", sig))
		}
	}
}
