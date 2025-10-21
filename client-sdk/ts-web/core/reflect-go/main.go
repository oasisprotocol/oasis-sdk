// Package main generates TypeScript client bindings from Oasis Core Go types
// and APIs.
package main

import (
	"context"
	"fmt"
	"go/ast"
	"go/doc"
	"go/token"
	"go/types"
	"io"
	"os"
	"path"
	"reflect"
	"regexp"
	"runtime"
	"sort"
	"strings"
	"sync"
	"time"
	_ "unsafe"

	"golang.org/x/tools/go/packages" // nolint:depguard

	beacon "github.com/oasisprotocol/oasis-core/go/beacon/api"
	"github.com/oasisprotocol/oasis-core/go/common/cbor"
	"github.com/oasisprotocol/oasis-core/go/common/crypto/signature"
	"github.com/oasisprotocol/oasis-core/go/common/node"
	"github.com/oasisprotocol/oasis-core/go/common/pubsub"
	"github.com/oasisprotocol/oasis-core/go/common/quantity"
	"github.com/oasisprotocol/oasis-core/go/common/version"
	consensus "github.com/oasisprotocol/oasis-core/go/consensus/api"
	"github.com/oasisprotocol/oasis-core/go/consensus/api/transaction"
	control "github.com/oasisprotocol/oasis-core/go/control/api"
	governance "github.com/oasisprotocol/oasis-core/go/governance/api"
	keymanager "github.com/oasisprotocol/oasis-core/go/keymanager/api"
	registry "github.com/oasisprotocol/oasis-core/go/registry/api"
	roothash "github.com/oasisprotocol/oasis-core/go/roothash/api"
	runtimeClient "github.com/oasisprotocol/oasis-core/go/runtime/client/api"
	scheduler "github.com/oasisprotocol/oasis-core/go/scheduler/api"
	staking "github.com/oasisprotocol/oasis-core/go/staking/api"
	storage "github.com/oasisprotocol/oasis-core/go/storage/api"
	"github.com/oasisprotocol/oasis-core/go/storage/mkvs/syncer"
	workerStorage "github.com/oasisprotocol/oasis-core/go/worker/storage/api"
)

type usedType struct {
	ref    string
	source string
}

type clientCode struct {
	methodDescriptors string
	methods           string
}

var (
	used      = []*usedType{}
	usedNames = map[string]bool{}
	memo      = map[reflect.Type]*usedType{}
)

func collectPath(fn any, stripComponents string) string {
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
	"github.com/oasisprotocol/oasis-core/go": collectPath(version.Version.String, "/common/version/version.go"),
}

var (
	modulePathsConsulted = map[string]bool{}
	packageTypes         = map[string]map[string]*doc.Type{}
	packageInfos         = map[string]*types.Info{}
	packageTypeSpecs     = map[string]map[types.Type]*ast.TypeSpec{}
)

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
	cfg := &packages.Config{
		Mode: packages.NeedSyntax | packages.NeedTypes | packages.NeedTypesInfo,
		Dir:  pkgPath,
	}
	pkgs, err := packages.Load(cfg, "./...")
	if err != nil {
		panic(err)
	}
	var (
		fset  *token.FileSet
		info  *types.Info
		files []*ast.File
	)
	for _, pkg := range pkgs {
		fset = pkg.Fset
		info = pkg.TypesInfo
		files = append(files, pkg.Syntax...)
	}
	dpkg, err := doc.NewFromFiles(fset, files, importPath)
	if err != nil {
		panic(err)
	}
	typesByName := make(map[string]*doc.Type)
	for _, dt := range dpkg.Types {
		typesByName[dt.Name] = dt
	}
	typeSpecs := make(map[types.Type]*ast.TypeSpec)
	for _, file := range files {
		for _, decl := range file.Decls {
			gen, ok := decl.(*ast.GenDecl)
			if !ok || gen.Tok != token.TYPE {
				continue
			}
			for _, spec := range gen.Specs {
				ts, ok := spec.(*ast.TypeSpec)
				if !ok {
					panic(fmt.Sprintf("expected type spec for %v", spec))
				}
				if typ := info.Defs[ts.Name]; typ != nil {
					typeSpecs[typ.Type()] = ts
				}
			}
		}
	}
	packageTypes[importPath] = typesByName
	packageInfos[importPath] = info
	packageTypeSpecs[importPath] = typeSpecs
}

func getTypeDoc(t reflect.Type) string {
	parseDocs(t.PkgPath())
	if dt, ok := packageTypes[t.PkgPath()][t.Name()]; ok {
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

var typeMethods = map[string]map[string]*ast.Field{}

func getMethodLookupVisitEmbedded(methodsByName map[string]*ast.Field, elem *ast.Field, info *types.Info, typeSpecs map[types.Type]*ast.TypeSpec) {
	_, _ = fmt.Fprintf(os.Stderr, "visiting doc interface element %v\n", elem)
	switch t := elem.Type.(type) {
	case *ast.Ident:
		typ := info.TypeOf(t)
		named, ok := typ.(*types.Named)
		if !ok {
			panic(fmt.Sprintf("expected named type for %v", t))
		}
		ts, ok := typeSpecs[named]
		if !ok {
			panic(fmt.Sprintf("type spec not found for %v", t))
		}
		getMethodLookupVisitInterface(methodsByName, ts, info, typeSpecs)
	case *ast.SelectorExpr:
		typ := info.TypeOf(t)
		named, ok := typ.(*types.Named)
		if !ok {
			panic(fmt.Sprintf("expected named type for %v", t))
		}
		importedPkg := named.Obj().Pkg()
		pkgPath := importedPkg.Path()
		parseDocs(pkgPath)
		dt := packageTypes[pkgPath][t.Sel.Name]
		info = packageInfos[pkgPath]
		typeSpecs = packageTypeSpecs[pkgPath]
		getMethodLookupVisitInterface(methodsByName, dt.Decl.Specs[0].(*ast.TypeSpec), info, typeSpecs)
	default:
		panic(fmt.Sprintf("method %v unexpected type", elem))
	}
}

func getMethodLookupVisitInterface(methodsByName map[string]*ast.Field, ts *ast.TypeSpec, info *types.Info, typeSpec map[types.Type]*ast.TypeSpec) {
	_, _ = fmt.Fprintf(os.Stderr, "visiting doc interface %v\n", ts)
	it := ts.Type.(*ast.InterfaceType)
	for _, elem := range it.Methods.List {
		if _, ok := elem.Type.(*ast.FuncType); ok {
			if len(elem.Names) > 1 {
				panic(fmt.Sprintf("method %v unexpected multiple names", elem))
			}
			if len(elem.Names) == 0 {
				continue
			}
			methodsByName[elem.Names[0].Name] = elem
		} else {
			getMethodLookupVisitEmbedded(methodsByName, elem, info, typeSpec)
		}
	}
}

func getMethodLookup(t reflect.Type) map[string]*ast.Field {
	sig := fmt.Sprintf("%s.%s", t.PkgPath(), t.Name())
	if methodsByName, ok := typeMethods[sig]; ok {
		return methodsByName
	}
	methodsByName := make(map[string]*ast.Field)
	parseDocs(t.PkgPath())
	dt := packageTypes[t.PkgPath()][t.Name()]
	info := packageInfos[t.PkgPath()]
	typeSpec := packageTypeSpecs[t.PkgPath()]
	getMethodLookupVisitInterface(methodsByName, dt.Decl.Specs[0].(*ast.TypeSpec), info, typeSpec)
	typeMethods[sig] = methodsByName
	return methodsByName
}

func getMethodDoc(t reflect.Type, name string) string {
	method, ok := getMethodLookup(t)[name]
	if !ok {
		panic(fmt.Sprintf("source for %s %v method %s not found", t.PkgPath(), t, name))
	}
	return method.Doc.Text()
}

var patternLine = regexp.MustCompile(`(?m)^(.?)`)

func renderDocComment(godoc string, indent string) string {
	if godoc == "" {
		return ""
	}
	indented := patternLine.ReplaceAllStringFunc(godoc, func(s string) string {
		if len(s) > 0 {
			return indent + " * " + s
		}
		return indent + " *"
	})
	return indent + "/**\n" + indented + "/\n"
}

func getMethodArgName(t reflect.Type, name string, i int) string {
	method, ok := getMethodLookup(t)[name]
	if !ok {
		panic(fmt.Sprintf("source for %s %v method %s not found", t.PkgPath(), t, name))
	}
	arg := method.Type.(*ast.FuncType).Params.List[i]
	if len(arg.Names) > 1 {
		panic(fmt.Sprintf("arg %v unexpected multiple names", arg))
	}
	if len(arg.Names) == 0 {
		return ""
	}
	return arg.Names[0].Name
}

var customStructNames = map[reflect.Type]string{
	reflect.TypeOf(consensus.Parameters{}): "ConsensusLightParameters",
}
var customStructNamesConsulted = map[reflect.Type]struct{}{}

var prefixByPackage = map[string]string{
	"github.com/oasisprotocol/oasis-core/go/beacon":                  "Beacon",
	"github.com/oasisprotocol/oasis-core/go/common/cbor":             "CBOR",
	"github.com/oasisprotocol/oasis-core/go/common/crypto/signature": "Signature",
	"github.com/oasisprotocol/oasis-core/go/common/entity":           "Entity",
	"github.com/oasisprotocol/oasis-core/go/common/node":             "Node",
	"github.com/oasisprotocol/oasis-core/go/common/sgx":              "SGX",
	"github.com/oasisprotocol/oasis-core/go/common/sgx/ias":          "SGXIas",
	"github.com/oasisprotocol/oasis-core/go/common/sgx/pcs":          "SGXPcs",
	"github.com/oasisprotocol/oasis-core/go/common/version":          "Version",
	"github.com/oasisprotocol/oasis-core/go/consensus":               "Consensus",
	"github.com/oasisprotocol/oasis-core/go/control":                 "Control",
	"github.com/oasisprotocol/oasis-core/go/genesis":                 "Genesis",
	"github.com/oasisprotocol/oasis-core/go/governance":              "Governance",
	"github.com/oasisprotocol/oasis-core/go/keymanager":              "KeyManager",
	"github.com/oasisprotocol/oasis-core/go/keymanager/churp":        "KeyManagerCHURP",
	"github.com/oasisprotocol/oasis-core/go/keymanager/secrets":      "KeyManagerSecrets",
	"github.com/oasisprotocol/oasis-core/go/p2p":                     "P2P",
	"github.com/oasisprotocol/oasis-core/go/registry":                "Registry",
	"github.com/oasisprotocol/oasis-core/go/roothash":                "RootHash",
	"github.com/oasisprotocol/oasis-core/go/runtime/client":          "RuntimeClient",
	"github.com/oasisprotocol/oasis-core/go/runtime/host":            "RuntimeHost",
	"github.com/oasisprotocol/oasis-core/go/runtime/bundle":          "RuntimeBundle",
	"github.com/oasisprotocol/oasis-core/go/runtime/history":         "RuntimeHistory",
	"github.com/oasisprotocol/oasis-core/go/scheduler":               "Scheduler",
	"github.com/oasisprotocol/oasis-core/go/staking":                 "Staking",
	"github.com/oasisprotocol/oasis-core/go/storage":                 "Storage",
	"github.com/oasisprotocol/oasis-core/go/upgrade":                 "Upgrade",
	"github.com/oasisprotocol/oasis-core/go/vault":                   "Vault",
	"github.com/oasisprotocol/oasis-core/go/worker/common":           "WorkerCommon",
	"github.com/oasisprotocol/oasis-core/go/worker/storage":          "WorkerStorage",
	"github.com/oasisprotocol/oasis-core/go/worker/compute":          "WorkerCompute",
	"github.com/oasisprotocol/oasis-core/go/worker/keymanager":       "WorkerKeyManager",
}
var prefixConsulted = map[string]struct{}{}

func getPrefix(t reflect.Type) string {
	pkgDir := t.PkgPath()

	for {
		if pkgDir == "." {
			panic(fmt.Sprintf("unset package prefix %s", t.PkgPath()))
		}

		if prefix, ok := prefixByPackage[pkgDir]; ok {
			prefixConsulted[pkgDir] = struct{}{}
			return prefix
		}

		pkgDir = path.Dir(pkgDir)
	}
}

func getStructName(t reflect.Type) string {
	if ref, ok := customStructNames[t]; ok {
		customStructNamesConsulted[t] = struct{}{}
		return ref
	}
	prefix := getPrefix(t)
	if prefix == t.Name() {
		return t.Name()
	}
	return prefix + t.Name()
}

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

func visitSigned(t reflect.Type) {
	_, _ = fmt.Fprintf(os.Stderr, "visiting signed wrapper %v\n", t)
	m, ok := reflect.PointerTo(t).MethodByName("Open")
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
		visitType(u.Elem(), false)
		outParams++
	}
	if outParams != 1 {
		panic("wrong number of out params")
	}
}

func importedRef(ref string, typesDot bool) string {
	if typesDot {
		return "types." + ref
	}
	return ref
}

const (
	structModeObject   = "object"
	structModeArray    = "array"
	structModeEmptyMap = "empty-map"
)

func visitStruct(t reflect.Type) string { //nolint: gocyclo
	if ut, ok := memo[t]; ok {
		return ut.ref
	}
	sourceDoc := renderDocComment(getTypeDoc(t), "")
	ref := getStructName(t)
	if usedNames[ref] {
		panic(fmt.Sprintf("name collision %s", ref))
	}
	var extendsType reflect.Type
	extendsRef := ""
	sourceExtends := ""
	sourceFields := ""
	mode := structModeObject
	for i := 0; i < t.NumField(); i++ {
		f := t.Field(i)
		_, _ = fmt.Fprintf(os.Stderr, "visiting field %v\n", f)
		if f.Anonymous {
			if extendsType == nil {
				extendsType = f.Type
				extendsRef = visitType(extendsType, false)
				sourceExtends = fmt.Sprintf(" extends %s", extendsRef)
			} else {
				panic("multiple embedded types")
			}
			continue
		}
		var name string
		var optional string
		if cborTag, ok := f.Tag.Lookup("cbor"); ok { //nolint: nestif
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
						mode = structModeArray
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
		case structModeObject:
			sourceFields += fmt.Sprintf("%s    %s%s: %s;\n", sourceFieldDoc, name, optional, visitType(f.Type, false))
		case structModeArray:
			if optional != "" {
				panic("unhandled optional in mode array")
			}
			sourceFields += fmt.Sprintf("%s    %s: %s,\n", sourceFieldDoc, name, visitType(f.Type, false))
		default:
			panic(fmt.Sprintf("unhandled struct field in mode %s", mode))
		}
	}
	if t == reflect.TypeOf(registry.VersionInfo{}) {
		// `.TEE` contains serialized `Constraints` for use with detached
		// signature
		visitType(reflect.TypeOf(node.SGXConstraints{}), false)
		encounteredVersionInfo = true
	}
	if sourceFields == "" && extendsType != nil {
		// there's a convention where we have a struct that wraps
		// `signature.Signed` with an `Open` method that has an out
		// pointer to the type of the signed data.
		if extendsType == reflect.TypeOf(signature.Signed{}) {
			visitSigned(t)
		}
		return extendsRef
	}
	if mode == structModeObject && sourceFields == "" && extendsType == nil {
		mode = structModeEmptyMap
	}
	var source string
	switch mode {
	case structModeObject:
		source = fmt.Sprintf("%sexport interface %s%s {\n%s}\n", sourceDoc, ref, sourceExtends, sourceFields)
	case structModeArray:
		if extendsType != nil {
			panic("unhandled extends in mode array")
		}
		source = fmt.Sprintf("%sexport type %s = [\n%s];\n", sourceDoc, ref, sourceFields)
	case structModeEmptyMap:
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
	usedNames[ref] = true
	memo[t] = &ut
	return ref
}

func visitType(t reflect.Type, typesDot bool) string {
	_, _ = fmt.Fprintf(os.Stderr, "visiting type %v\n", t)
	switch t {
	case reflect.TypeOf(time.Time{}):
		t = reflect.TypeOf(int64(0))
	case reflect.TypeOf(quantity.Quantity{}):
		t = reflect.TypeOf([]byte{})
	case reflect.TypeOf(cbor.RawMessage{}):
		return "unknown"
	}
	switch t.Kind() {
	// Invalid begone
	case reflect.Bool:
		return "boolean"
	case reflect.Int, reflect.Int8, reflect.Int16, reflect.Int32, reflect.Uint, reflect.Uint8, reflect.Uint16, reflect.Uint32:
		return "number"
	case reflect.Int64, reflect.Uint64, reflect.Uintptr:
		return importedRef("longnum", typesDot)
	case reflect.Float32, reflect.Float64:
		return "number"
	// Complex64, Complex128 begone
	case reflect.Array, reflect.Slice:
		if t.Elem().Kind() == reflect.Uint8 {
			return "Uint8Array"
		}
		return fmt.Sprintf("%s[]", visitType(t.Elem(), typesDot))
	// Chan begone
	// Func begone
	// Interface begone
	case reflect.Map:
		if t.Key().Kind() == reflect.String {
			return fmt.Sprintf("{[%s: string]: %s}", getMapKeyName(t), visitType(t.Elem(), typesDot))
		}
		return fmt.Sprintf("Map<%s, %s>", visitType(t.Key(), typesDot), visitType(t.Elem(), typesDot))
	case reflect.Ptr:
		return visitType(t.Elem(), typesDot)
	case reflect.String:
		return "string"
	case reflect.Struct:
		return importedRef(visitStruct(t), typesDot)
	// UnsafePointer begone
	default:
		panic(fmt.Sprintf("unhandled kind %v", t.Kind()))
	}
}

const (
	descriptorKindUnary           = "Unary"
	descriptorKindServerStreaming = "ServerStreaming"
)

var skipMethods = map[string]struct{}{
	"github.com/oasisprotocol/oasis-core/go/storage/api.Backend.Initialized": {},
	// getters for other APIs
	"github.com/oasisprotocol/oasis-core/go/keymanager/api.Backend.Secrets":         {},
	"github.com/oasisprotocol/oasis-core/go/keymanager/api.Backend.Churp":           {},
	"github.com/oasisprotocol/oasis-core/go/consensus/api.Backend.State":            {},
	"github.com/oasisprotocol/oasis-core/go/runtime/client/api.RuntimeClient.State": {},
}
var skipMethodsConsulted = map[string]struct{}{}

func (c *clientCode) visitClientWithService(i any, service string, methodPrefix string) {
	t := reflect.TypeOf(i).Elem()
	_, _ = fmt.Fprintf(os.Stderr, "visiting client %v\n", t)
	for i := 0; i < t.NumMethod(); i++ {
		m := t.Method(i)
		c.visitMethodWithService(t, m, service, methodPrefix)
	}
	c.methodDescriptors += "\n"
}

func (c *clientCode) visitMethodWithService(t reflect.Type, m reflect.Method, service string, methodPrefix string) {
	_, _ = fmt.Fprintf(os.Stderr, "visiting method %v\n", m)
	sig := fmt.Sprintf("%s.%s.%s", t.PkgPath(), t.Name(), m.Name)
	if _, ok := skipMethods[sig]; ok {
		skipMethodsConsulted[sig] = struct{}{}
		return
	}
	descriptorKind := descriptorKindUnary
	var inArgIndex int
	var inRef string
	var outRef string
	for j := 0; j < m.Type.NumIn(); j++ {
		u := m.Type.In(j)
		// skip context
		if u == reflect.TypeOf((*context.Context)(nil)).Elem() {
			continue
		}
		// writer means streaming byte array output
		if u == reflect.TypeOf((*io.Writer)(nil)).Elem() {
			descriptorKind = descriptorKindServerStreaming
			outRef = visitType(reflect.TypeOf([]byte{}), true)
			continue
		}
		if inRef != "" {
			_, _ = fmt.Fprintf(os.Stderr, "type %v method %v unexpected multiple in types\n", t, m)
		}
		inArgIndex = j
		inRef = visitType(u, true)
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
		if outRef != "" {
			_, _ = fmt.Fprintf(os.Stderr, "type %v method %v unexpected multiple out types\n", t, m)
		}
		// visit sync chunk instead
		if u == reflect.TypeOf((*storage.WriteLogIterator)(nil)).Elem() {
			u = reflect.TypeOf(storage.SyncChunk{})
			descriptorKind = descriptorKindServerStreaming
		}
		// visit stream datum instead
		if u.Kind() == reflect.Chan {
			u = u.Elem()
			descriptorKind = descriptorKindServerStreaming
		}
		outRef = visitType(u, true)
	}
	var inParam, inArg string
	if inRef == "" {
		inRef = "void"
		inArg = "undefined"
	} else {
		inArg = getMethodArgName(t, m.Name, inArgIndex)
		if inArg == "" {
			// why didn't we put the name in the interface spec ugh
			switch m.Type.In(inArgIndex) {
			case reflect.TypeOf(uint64(0)), reflect.TypeOf(int64(0)):
				// oh my god our codebase
				inArg = "height"
			case reflect.TypeOf(beacon.EpochTime(0)):
				inArg = "epoch"
			default:
				inArg = "query"
			}
		}
		inParam = inArg + ": " + inRef
	}
	if outRef == "" {
		outRef = "void"
	}
	methodDoc := renderDocComment(getMethodDoc(t, m.Name), "    ")
	lowerService := strings.ToLower(service[:1]) + service[1:]
	c.methodDescriptors += fmt.Sprintf("const methodDescriptor%s%s%s = createMethodDescriptor%s<%s, %s>('%s', '%s%s');\n", service, methodPrefix, m.Name, descriptorKind, inRef, outRef, service, methodPrefix, m.Name)
	c.methods += fmt.Sprintf("%s    %s%s%s(%s) { return this.call%s(methodDescriptor%s%s%s, %s); }\n", methodDoc, lowerService, methodPrefix, m.Name, inParam, descriptorKind, service, methodPrefix, m.Name, inArg)
	c.methods += "\n"
}

func (c *clientCode) visitClient(i any) {
	t := reflect.TypeOf(i).Elem()
	prefix := getPrefix(t)
	c.visitClientWithService(i, prefix, "")
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

func (c *clientCode) writeClient(className string) {
	fmt.Print(c.methodDescriptors)
	fmt.Printf("export class %s extends GRPCWrapper {\n", className)
	fmt.Print(c.methods)
	fmt.Print("}\n\n")
}

// might be nicer to add a function to list these in oasis-core
//
//go:linkname registeredMethods github.com/oasisprotocol/oasis-core/go/consensus/api/transaction.registeredMethods
var registeredMethods sync.Map

func main() {
	var internal clientCode
	internal.visitClient((*beacon.Backend)(nil))
	internal.visitClient((*scheduler.Backend)(nil))
	internal.visitClient((*registry.Backend)(nil))
	internal.visitClient((*staking.Backend)(nil))
	internal.visitClient((*keymanager.Backend)(nil))
	internal.visitClient((*roothash.Backend)(nil))
	internal.visitClient((*governance.Backend)(nil))
	internal.visitClient((*storage.Backend)(nil))
	internal.visitClientWithService((*workerStorage.StorageWorker)(nil), "StorageWorker", "")
	internal.visitClient((*runtimeClient.RuntimeClient)(nil))
	internal.visitClient((*consensus.Backend)(nil))
	internal.visitClientWithService((*syncer.ReadSyncer)(nil), "Consensus", "State")
	internal.visitClientWithService((*control.NodeController)(nil), "NodeController", "")
	internal.visitClientWithService((*control.DebugController)(nil), "DebugController", "")

	_, _ = fmt.Fprintf(os.Stderr, "visiting transaction body types\n")
	registeredMethods.Range(func(name, bodyType any) bool {
		_, _ = fmt.Fprintf(os.Stderr, "visiting method %v\n", name)
		visitType(reflect.TypeOf(bodyType), false)
		return true
	})

	write()
	internal.writeClient("NodeInternal")
	for p := range modulePaths {
		if !modulePathsConsulted[p] {
			panic(fmt.Sprintf("unused module path %s", p))
		}
	}
	for t := range customStructNames {
		if _, ok := customStructNamesConsulted[t]; !ok {
			panic(fmt.Sprintf("unused custom type name %v", t))
		}
	}
	for prefix := range prefixByPackage {
		if _, ok := prefixConsulted[prefix]; !ok {
			panic(fmt.Sprintf("unused prefix %s", prefix))
		}
	}
	if !encounteredVersionInfo {
		panic("VersionInfo special case not needed")
	}
	for sig := range skipMethods {
		if _, ok := skipMethodsConsulted[sig]; !ok {
			panic(fmt.Sprintf("unused skip method %s", sig))
		}
	}
}
