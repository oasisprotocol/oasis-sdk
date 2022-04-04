package parsers

import (
	"fmt"
	"log"
	"os"
	"path/filepath"
	"regexp"
	"strings"

	"github.com/oasisprotocol/oasis-sdk/tools/extract-runtime-txs/types"
)

var TypeScriptWarnings = []error{}

type TypeScriptParser struct {
	filename  string
	txs       map[string]string
	txParams  map[string]string
	txResults map[string]string
}

func NewTypeScriptParser(filename string) TypeScriptParser {
	return TypeScriptParser{
		filename:  filename,
		txs:       map[string]string{},
		txParams:  map[string]string{},
		txResults: map[string]string{},
	}
}

func PopulateTypeScriptRefs(transactions map[string]types.Tx, searchDir string) error {
	err := filepath.Walk(searchDir, func(path string, f os.FileInfo, err error) error {
		if err != nil {
			log.Fatal(err)
		}
		if f.IsDir() {
			return nil
		}
		// Ts source files only, ignore types.ts, because we parse it indirectly.
		if !strings.HasSuffix(f.Name(), ".ts") || f.Name() == "types.ts" {
			return nil
		}
		tsParser := NewTypeScriptParser(path)
		e := tsParser.populateTransactionRefs(transactions)
		if e != nil {
			return e
		}

		return nil
	})
	if err != nil {
		return err
	}

	return nil
}

func (p *TypeScriptParser) populateTransactionRefs(txs map[string]types.Tx) error {
	text, err := readFile(p.filename)
	if err != nil {
		return err
	}

	// export const METHOD_SOME_METHOD = 'some_module.SomeMethod';
	regMethodMatch, _ := regexp.Compile("export const METHOD_.+ = '([a-zA-Z_\\.]+)'")
	// callSomeMethod() { or querySomeMethod() {
	regCallQueryMatch, _ := regexp.Compile("    (call|query)(.+)\\(\\) \\{")
	// callSomeMethod() { or querySomeMethod() {
	regTxTypesMatch, _ := regexp.Compile("        return this\\.(call|query)<(.+), (.+)>")

	// Collect name -> fullName of transactions in this file.
	// Collect TxTypeName -> fullName of transaction params and results in the file.
	for lineIdx := 0; lineIdx < len(text); lineIdx += 1 {
		methodMatch := regMethodMatch.FindStringSubmatch(text[lineIdx])
		if len(methodMatch) > 0 {
			fullNameSplit := strings.Split(methodMatch[1], ".")
			_, found := txs[methodMatch[1]]
			if !found {
				TypeScriptWarnings = append(TypeScriptWarnings, fmt.Errorf("unknown method %s in file %s:%d", methodMatch[1], p.filename, lineIdx+1))
			}
			p.txs[fullNameSplit[1]] = methodMatch[1]
		}

		callQueryMatch := regCallQueryMatch.FindStringSubmatch(text[lineIdx])
		if len(callQueryMatch) == 3 {
			fullName, valid := p.txs[callQueryMatch[2]]
			if !valid {
				TypeScriptWarnings = append(TypeScriptWarnings, fmt.Errorf("implementation of %s not defined as method in the beginning of %s", callQueryMatch[2], p.filename))
				continue
			}
			if _, valid = txs[fullName]; !valid {
				continue
			}

			txTypesMatch := regTxTypesMatch.FindStringSubmatch(text[lineIdx+1])
			if len(txTypesMatch) == 4 {
				if strings.HasPrefix(txTypesMatch[2], "types.") {
					name := strings.TrimPrefix(txTypesMatch[2], "types.")
					p.txParams[name] = fullName
				}
				if strings.HasPrefix(txTypesMatch[3], "types.") {
					name := strings.TrimPrefix(txTypesMatch[3], "types.")
					p.txResults[name] = fullName
				}
			}

			_, lineFrom := findComment(text, lineIdx, "    ")
			lineTo, err := findEndBlock(text, lineIdx, "    ", fullName)
			if err != nil {
				return err
			}
			txs[fullName].Ref[types.TypeScript] = types.Snippet{
				Path:     p.filename,
				LineFrom: lineFrom,
				LineTo:   lineTo,
			}
		}
	}

	// Open types.ts of the same module and collect parameters and result snippets.
	if len(p.txs) > 0 {
		if err := p.populateParamsResultRefs(txs); err != nil {
			return err
		}
	}

	return nil
}

// populateParamsResultRefs opens types.ts file, finds corresponding parameters and results snippets
// for the provided transactions and populates the refs of global transactions.
func (p *TypeScriptParser) populateParamsResultRefs(txs map[string]types.Tx) error {
	typesPath := filepath.Join(filepath.Dir(p.filename), "types.ts")
	text, err := readFile(typesPath)
	if err != nil {
		return err
	}

	regTypeMatch, _ := regexp.Compile("export interface ([a-zA-Z]+)")
	for lineIdx := 0; lineIdx < len(text); lineIdx += 1 {
		typeMatch := regTypeMatch.FindStringSubmatch(text[lineIdx])
		if len(typeMatch) > 0 {
			_, lineFrom := findComment(text, lineIdx, "")
			lineTo, err := findEndBlock(text, lineIdx, "", typeMatch[1])
			if err != nil {
				return err
			}

			snippet := types.Snippet{
				Path:     typesPath,
				LineFrom: lineFrom,
				LineTo:   lineTo,
			}
			if fullName, valid := p.txParams[typeMatch[1]]; valid {
				txs[fullName].ParametersRef[types.TypeScript] = snippet
			} else if fullName, valid := p.txResults[typeMatch[1]]; valid {
				txs[fullName].ResultRef[types.TypeScript] = snippet
			}
		}
	}

	return nil
}
