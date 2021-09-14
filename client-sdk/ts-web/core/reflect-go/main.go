package main

import (
	"context"
	"fmt"
	"go/ast"
	"go/doc"
	"go/parser"
	"go/token"
	"io"
	"net"
	"os"
	"path"
	"reflect"
	"regexp"
	"runtime"
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
	"github.com/oasisprotocol/oasis-core/go/common/sgx"
	"github.com/oasisprotocol/oasis-core/go/common/version"
	consensus "github.com/oasisprotocol/oasis-core/go/consensus/api"
	"github.com/oasisprotocol/oasis-core/go/consensus/api/transaction"
	control "github.com/oasisprotocol/oasis-core/go/control/api"
	governance "github.com/oasisprotocol/oasis-core/go/governance/api"
	keymanager "github.com/oasisprotocol/oasis-core/go/keymanager/api"
	registry "github.com/oasisprotocol/oasis-core/go/registry/api"
	roothash "github.com/oasisprotocol/oasis-core/go/roothash/api"
	"github.com/oasisprotocol/oasis-core/go/roothash/api/commitment"
	runtimeClient "github.com/oasisprotocol/oasis-core/go/runtime/client/api"
	enclaverpc "github.com/oasisprotocol/oasis-core/go/runtime/enclaverpc/api"
	scheduler "github.com/oasisprotocol/oasis-core/go/scheduler/api"
	staking "github.com/oasisprotocol/oasis-core/go/staking/api"
	storage "github.com/oasisprotocol/oasis-core/go/storage/api"
	workerStorage "github.com/oasisprotocol/oasis-core/go/worker/storage/api"
)

type usedType struct {
	ref    string
	source string
}

var used = []*usedType{}
var memo = map[reflect.Type]*usedType{}

func collectPath(fn interface{}, stripComponents string) string {
	// the idea of sneaking in through runtime.FuncForPC from https://stackoverflow.com/a/54588577/1864688
	reflectFn := reflect.ValueOf(fn)
	entryPoint := reflectFn.Pointer()
	runtimeFn := runtime.FuncForPC(entryPoint)
	file, _ := runtimeFn.FileLine(entryPoint)
	splitIdx := len(file) - len(stripComponents)
	if file[splitIdx:] != stripComponents {
		panic(fmt.Sprintf("fn %v file %s does not end with %s", fn, file, stripComponents))
	}
	return file[:splitIdx]
}

var modulePaths = map[string]string{
	"net": collectPath((*net.TCPAddr).String, "/tcpsock.go"),

	"github.com/oasisprotocol/oasis-core/go": collectPath(version.Version.String, "/common/version/version.go"),
}
var modulePathsConsulted = map[string]bool{}
var packageTypes = map[string]map[string]*doc.Type{}

func parseDocs(importPath string) {
	if _, ok := packageTypes[importPath]; ok {
		return
	}
	module := importPath
	var pkgPath string
	for {
		if module == "." {
			panic(fmt.Sprintf("package %s path not known", importPath))
		}
		if modulePath, ok := modulePaths[module]; ok {
			modulePathsConsulted[module] = true
			pkgPath = modulePath + importPath[len(module):]
			break
		}
		module = path.Dir(module)
	}
	fset := token.NewFileSet()
	pkgs, err := parser.ParseDir(fset, pkgPath, nil, parser.ParseComments)
	if err != nil {
		panic(err)
	}
	var files []*ast.File
	for _, pkg := range pkgs {
		for _, file := range pkg.Files {
			files = append(files, file)
		}
	}
	dpkg, err := doc.NewFromFiles(fset, files, importPath)
	if err != nil {
		panic(err)
	}
	typesByName := make(map[string]*doc.Type)
	for _, dt := range dpkg.Types {
		typesByName[dt.Name] = dt
	}
	packageTypes[importPath] = typesByName
}

func getTypeDoc(t reflect.Type) string {
	parseDocs(t.PkgPath())
	typesByName := packageTypes[t.PkgPath()]
	if dt, ok := typesByName[t.Name()]; ok {
		return dt.Doc
	}
	return ""
}

var typeFields = map[string]map[string]*ast.Field{}

func getFieldLookup(t reflect.Type) map[string]*ast.Field {
	sig := fmt.Sprintf("%s.%s", t.PkgPath(), t.Name())
	if fieldsByName, ok := typeFields[sig]; ok {
		return fieldsByName
	}
	fieldsByName := make(map[string]*ast.Field)
	parseDocs(t.PkgPath())
	dt := packageTypes[t.PkgPath()][t.Name()]
	fields := dt.Decl.Specs[0].(*ast.TypeSpec).Type.(*ast.StructType).Fields
	for _, field := range fields.List {
		if len(field.Names) > 1 {
			panic(fmt.Sprintf("type %v field %v unexpected multiple names", t, field))
		}
		if len(field.Names) == 0 {
			continue
		}
		fieldsByName[field.Names[0].Name] = field
	}
	typeFields[sig] = fieldsByName
	return fieldsByName
}

func getFieldDoc(t reflect.Type, name string) string {
	field, ok := getFieldLookup(t)[name]
	if !ok {
		panic(fmt.Sprintf("source for %v field %s not found", t, name))
	}
	return field.Doc.Text()
}

func renderDocComment(godoc string, indent string) string {
	if godoc == "" {
		return ""
	}
	indented := regexp.MustCompile(`(?m)^(.?)`).ReplaceAllStringFunc(godoc, func(s string) string {
		if len(s) > 0 {
			return indent + " * " + s
		}
		return indent + " *"
	})
	return indent + "/**\n" + indented + "/\n"
}

var customStructNames = map[reflect.Type]string{
	reflect.TypeOf(consensus.Parameters{}): "ConsensusLightParameters",
}
var customStructNamesConsulted = map[reflect.Type]bool{}

var prefixByPackage = map[string]string{
	"net": "Net",

	"github.com/oasisprotocol/oasis-core/go/beacon":                  "Beacon",
	"github.com/oasisprotocol/oasis-core/go/common/cbor":             "CBOR",
	"github.com/oasisprotocol/oasis-core/go/common/crypto/pvss":      "PVSS",
	"github.com/oasisprotocol/oasis-core/go/common/crypto/signature": "Signature",
	"github.com/oasisprotocol/oasis-core/go/common/entity":           "Entity",
	"github.com/oasisprotocol/oasis-core/go/common/node":             "Node",
	"github.com/oasisprotocol/oasis-core/go/common/sgx":              "SGX",
	"github.com/oasisprotocol/oasis-core/go/common/version":          "Version",
	"github.com/oasisprotocol/oasis-core/go/consensus":               "Consensus",
	"github.com/oasisprotocol/oasis-core/go/control":                 "Control",
	"github.com/oasisprotocol/oasis-core/go/genesis":                 "Genesis",
	"github.com/oasisprotocol/oasis-core/go/governance":              "Governance",
	"github.com/oasisprotocol/oasis-core/go/keymanager":              "KeyManager",
	"github.com/oasisprotocol/oasis-core/go/registry":                "Registry",
	"github.com/oasisprotocol/oasis-core/go/roothash":                "RootHash",
	"github.com/oasisprotocol/oasis-core/go/runtime/client":          "RuntimeClient",
	"github.com/oasisprotocol/oasis-core/go/runtime/enclaverpc":      "EnclaveRPC",
	"github.com/oasisprotocol/oasis-core/go/runtime/host":            "RuntimeHost",
	"github.com/oasisprotocol/oasis-core/go/scheduler":               "Scheduler",
	"github.com/oasisprotocol/oasis-core/go/staking":                 "Staking",
	"github.com/oasisprotocol/oasis-core/go/storage":                 "Storage",
	"github.com/oasisprotocol/oasis-core/go/upgrade":                 "Upgrade",
	"github.com/oasisprotocol/oasis-core/go/worker/common":           "WorkerCommon",
	"github.com/oasisprotocol/oasis-core/go/worker/storage":          "WorkerStorage",
}
var prefixConsulted = map[string]bool{}

func getStructName(t reflect.Type) string {
	if ref, ok := customStructNames[t]; ok {
		customStructNamesConsulted[t] = true
		return ref
	}
	pkgDir := t.PkgPath()
	var prefix string
	for {
		if pkgDir == "." {
			panic(fmt.Sprintf("unset package prefix %s", t.PkgPath()))
		}
		var ok bool
		prefix, ok = prefixByPackage[pkgDir]
		if ok {
			prefixConsulted[pkgDir] = true
			break
		}
		pkgDir = path.Dir(pkgDir)
	}
	if prefix == t.Name() {
		return t.Name()
	}
	return prefix + t.Name()
}

var mapKeyNames = map[reflect.Type]string{}
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

var encounteredVersionInfo = false
var encounteredExecutorCommitment = false

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
		sourceDoc := renderDocComment(getTypeDoc(t), "")
		ref := getStructName(t)
		var extendsType reflect.Type
		extendsRef := ""
		sourceExtends := ""
		sourceFields := ""
		mode := "object"
		for i := 0; i < t.NumField(); i++ {
			f := t.Field(i)
			_, _ = fmt.Fprintf(os.Stderr, "visiting field %v\n", f)
			if f.Anonymous {
				if extendsType == nil {
					extendsType = f.Type
					extendsRef = visitType(extendsType)
					sourceExtends = fmt.Sprintf(" extends %s", extendsRef)
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
			sourceFieldDoc := renderDocComment(getFieldDoc(t, f.Name), "    ")
			switch mode {
			case "object":
				sourceFields += fmt.Sprintf("%s    %s%s: %s;\n", sourceFieldDoc, name, optional, visitType(f.Type))
			case "array":
				if optional != "" {
					panic("unhandled optional in mode array")
				}
				sourceFields += fmt.Sprintf("%s    %s: %s,\n", sourceFieldDoc, name, visitType(f.Type))
			default:
				panic(fmt.Sprintf("unhandled struct field in mode %s", mode))
			}
		}
		if t == reflect.TypeOf(registry.VersionInfo{}) {
			// `.TEE` contains serialized `Constraints` for use with detached
			// signature
			visitType(reflect.TypeOf(sgx.Constraints{}))
			encounteredVersionInfo = true
		}
		if sourceFields == "" && extendsType != nil {
			// there's a convention where we have a struct that wraps
			// `signature.Signed` with an `Open` method that has an out
			// pointer to the type of the signed data.
			if extendsType == reflect.TypeOf(signature.Signed{}) {
				if t == reflect.TypeOf(commitment.ExecutorCommitment{}) {
					// this one is unconventional ):
					visitType(reflect.TypeOf(commitment.ComputeBody{}))
					encounteredExecutorCommitment = true
				} else {
					_, _ = fmt.Fprintf(os.Stderr, "visiting signed wrapper %v\n", t)
					m, ok := reflect.PtrTo(t).MethodByName("Open")
					if !ok {
						panic(fmt.Sprintf("signed wrapper %v has no open method", t))
					}
					_, _ = fmt.Fprintf(os.Stderr, "visiting open method %v\n", m)
					outParams := 0
					for i := 1; i < m.Type.NumIn(); i++ {
						u := m.Type.In(i)
						// skip parameters that couldn't be out pointers
						if u.Kind() != reflect.Ptr {
							continue
						}
						visitType(u.Elem())
						outParams++
					}
					if outParams != 1 {
						panic("wrong number of out params")
					}
				}
			}
			return extendsRef
		}
		if mode == "object" && sourceFields == "" && extendsType == nil {
			mode = "empty-map"
		}
		var source string
		switch mode {
		case "object":
			source = fmt.Sprintf("%sexport interface %s%s {\n%s}\n", sourceDoc, ref, sourceExtends, sourceFields)
		case "array":
			if extendsType != nil {
				panic("unhandled extends in mode array")
			}
			source = fmt.Sprintf("%sexport type %s = [\n%s];\n", sourceDoc, ref, sourceFields)
		case "empty-map":
			if extendsType != nil {
				panic("unhandled extends in mode empty-map")
			}
			if sourceFields != "" {
				panic("unhandled source fields in mode empty-map")
			}
			source = fmt.Sprintf("%sexport type %s = Map<never, never>;\n", sourceDoc, ref)
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
	"github.com/oasisprotocol/oasis-core/go/roothash/api.Backend.TrackRuntime":      true,
	"github.com/oasisprotocol/oasis-core/go/storage/api.Backend.Initialized":        true,
	"github.com/oasisprotocol/oasis-core/go/consensus/api.ClientBackend.Beacon":     true,
	"github.com/oasisprotocol/oasis-core/go/consensus/api.ClientBackend.Registry":   true,
	"github.com/oasisprotocol/oasis-core/go/consensus/api.ClientBackend.Staking":    true,
	"github.com/oasisprotocol/oasis-core/go/consensus/api.ClientBackend.Scheduler":  true,
	"github.com/oasisprotocol/oasis-core/go/consensus/api.ClientBackend.Governance": true,
	"github.com/oasisprotocol/oasis-core/go/consensus/api.ClientBackend.RootHash":   true,
	"github.com/oasisprotocol/oasis-core/go/consensus/api.ClientBackend.State":      true,
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
		fmt.Print("\n")
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
	visitType(reflect.TypeOf((*registry.DeregisterEntity)(nil)).Elem())
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
	for p := range modulePaths {
		if !modulePathsConsulted[p] {
			panic(fmt.Sprintf("unused module path %s", p))
		}
	}
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
	if !encounteredVersionInfo {
		panic("VersionInfo special case not needed")
	}
	if !encounteredExecutorCommitment {
		panic("ExecutorCommitment special case not needed")
	}
	for sig := range skipMethods {
		if !skipMethodsConsulted[sig] {
			panic(fmt.Sprintf("unused skip method %s", sig))
		}
	}
}
