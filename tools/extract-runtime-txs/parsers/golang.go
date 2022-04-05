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

type GolangParser struct {
	searchDir string
	warnings  []error

	filename string
	localTxs map[string]string
}

func NewGolangParser(searchDir string) *GolangParser {
	return &GolangParser{
		searchDir: searchDir,
		warnings:  []error{},
		localTxs:  map[string]string{},
	}
}

func (p *GolangParser) GenerateInitialTransactions() (map[string]types.Tx, error) {
	return nil, types.NotImplementedError
}

func (p *GolangParser) PopulateRefs(transactions map[string]types.Tx) error {
	err := filepath.Walk(p.searchDir, func(path string, f os.FileInfo, err error) error {
		if err != nil {
			log.Fatal(err)
		}
		if f.IsDir() {
			return nil
		}
		// Go source files only, ignore types.go, because we parse it indirectly.
		if !strings.HasSuffix(f.Name(), ".go") || f.Name() == "types.go" {
			return nil
		}

		p.filename = path
		e := p.populateTransactionRefs(transactions)
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

func (p *GolangParser) GetWarnings() []error {
	return p.warnings
}

func (p *GolangParser) populateTransactionRefs(txs map[string]types.Tx) error {
	p.localTxs = map[string]string{}

	text, err := readFile(p.filename)
	if err != nil {
		return err
	}

	// tab + methodSomeMethodName = "some_module.someName"
	regTxMatch, _ := regexp.Compile("\tmethod(.*) = \"([a-zA-Z_\\.]+)\"")
	// func (a *v1) SomeMethodName(...
	regImplMatch, _ := regexp.Compile("func \\(a \\*v1\\) ([a-zA-Z]+)\\(.*")

	// Collect name -> fullName of transactions in this file.
	for lineIdx := 0; lineIdx < len(text); lineIdx += 1 {
		txMatch := regTxMatch.FindStringSubmatch(text[lineIdx])
		if len(txMatch) > 0 {
			fullNameSplit := strings.Split(txMatch[2], ".")
			_, found := txs[txMatch[2]]
			if !found {
				p.warnings = append(p.warnings, fmt.Errorf("unknown method %s in file %s:%d", txMatch[2], p.filename, lineIdx+1))
			}
			p.localTxs[fullNameSplit[1]] = txMatch[2]
		}

		implMatch := regImplMatch.FindStringSubmatch(text[lineIdx])
		if len(implMatch) > 0 {
			fullName, valid := p.localTxs[implMatch[1]]
			if !valid {
				p.warnings = append(p.warnings, fmt.Errorf("implementation of %s not defined as method in the beginning of %s", implMatch[1], p.filename))
				continue
			}
			if _, valid = txs[fullName]; !valid {
				continue
			}

			_, lineFrom := findComment(text, lineIdx, "")
			lineTo, err := findEndBlock(text, lineIdx, "", fullName)
			if err != nil {
				return err
			}
			txs[fullName].Ref[types.Go] = types.Snippet{
				Path:     p.filename,
				LineFrom: lineFrom,
				LineTo:   lineTo,
			}
		}
	}

	// Open types.go of the same module and collect parameters and result snippets.
	if len(p.localTxs) > 0 {
		if err := p.populateParamsResultRefs(txs); err != nil {
			return err
		}
	}

	return nil
}

// populateParamsResultRefs opens types.go file in the current module's folder, finds parameters
// and results snippets for the provided transactions and populates the refs of global transactions.
func (p *GolangParser) populateParamsResultRefs(txs map[string]types.Tx) error {
	typesPath := filepath.Join(filepath.Dir(p.filename), "types.go")
	text, err := readFile(typesPath)
	if err != nil {
		return err
	}

	regTypeMatch, _ := regexp.Compile("type ([a-zA-Z]+) (.*)")
	for lineIdx := 0; lineIdx < len(text); lineIdx += 1 {
		typeMatch := regTypeMatch.FindStringSubmatch(text[lineIdx])
		if len(typeMatch) > 0 {
			result := strings.HasSuffix(typeMatch[1], "Result")
			name := strings.TrimSuffix(typeMatch[1], "Result")
			name = strings.TrimSuffix(name, "Query")

			fullName, valid := p.localTxs[name]
			if !valid {
				continue
			}
			if _, valid = txs[fullName]; !valid {
				continue
			}

			_, lineFrom := findComment(text, lineIdx, "")
			lineTo, err := findEndBlock(text, lineIdx, "", fullName)
			if err != nil {
				return err
			}

			snippet := types.Snippet{
				Path:     typesPath,
				LineFrom: lineFrom,
				LineTo:   lineTo,
			}
			if result {
				txs[fullName].ResultRef[types.Go] = snippet
			} else {
				txs[fullName].ParametersRef[types.Go] = snippet
			}
		}
	}

	return nil
}
