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

var RustWarnings = []error{}

type RustParser struct {
	filename string
}

// GenerateInitialTransactions generates a map of all runtime transactions found in the
// specified searchDir.
func GenerateInitialTransactions(searchDir string) (map[string]types.Tx, error) {
	transactions := map[string]types.Tx{}
	err := filepath.Walk(searchDir, func(path string, f os.FileInfo, err error) error {
		if err != nil {
			log.Fatal(err)
		}
		if f.IsDir() {
			return nil
		}
		if !strings.HasSuffix(f.Name(), ".rs") {
			return nil
		}
		rustParser := RustParser{filename: path}
		txs, err := rustParser.FindTransactions()
		if err != nil {
			return err
		}

		for _, tx := range txs {
			txOld, valid := transactions[tx.FullName()]
			if valid {
				return fmt.Errorf(
					"runtime transaction %s in %s:%d was already defined in %s:%d",
					tx.FullName(),
					tx.Ref[types.Rust].Path,
					tx.Ref[types.Rust].LineFrom,
					txOld.Ref[types.Rust].Path,
					txOld.Ref[types.Rust].LineFrom,
				)
			}
			transactions[tx.FullName()] = tx
		}

		return nil
	})
	if err != nil {
		return nil, err
	}

	return transactions, nil
}

func (r RustParser) getTypesFile() string {
	return filepath.Join(filepath.Dir(r.filename), "types.rs")
}

// FindTransactions scans the given rust source file and looks for the
// #[handler(call = "...")] or #[handler(query = "...")] pattern. For each
// matching pattern, a new Tx is added to the result set.
func (r RustParser) FindTransactions() ([]types.Tx, error) {
	text, err := readFile(r.filename)
	if err != nil {
		return nil, err
	}

	// some spaces + #[handler(txtype = "txfullname")]
	regMatch, _ := regexp.Compile("([ ]*)#\\[handler\\((call|query) = \"([a-zA-Z\\.]+)\"\\)\\]")

	txs := []types.Tx{}
	for lineIdx := 0; lineIdx < len(text); lineIdx += 1 {
		txMatch := regMatch.FindStringSubmatch(text[lineIdx])
		if len(txMatch) > 0 {
			// Check, if the function has a comment and include it.
			comment, lineFrom := findComment(text, lineIdx, txMatch[1])
			paramsName, resultName := r.findParamsResultName(text, lineIdx)

			txType := types.Call
			if txMatch[2] == string(types.Query) {
				txType = types.Query
			}

			fullNameSplit := strings.Split(txMatch[3], ".")

			lineTo, err := findEndBlock(text, lineIdx+1, txMatch[1], txMatch[3])
			if err != nil {
				return nil, err
			}

			parameters, paramSnippet, err := r.mustFindMembers(paramsName)
			if err != nil {
				return nil, err
			}

			var result []types.Parameter
			var resultSnippet *types.Snippet
			if resultName != "" {
				result, resultSnippet, err = r.findMembers(resultName)
				if err != nil {
					return nil, err
				}

				if resultSnippet == nil {
					RustWarnings = append(RustWarnings, fmt.Errorf("no definition found for %s in %s required by %s:%d", resultName, r.getTypesFile(), r.filename, lineIdx+1))
				}
			}
			tx := types.Tx{
				Module:  fullNameSplit[0],
				Name:    fullNameSplit[1],
				Comment: comment,
				Type:    txType,
				Ref: map[types.Lang]types.Snippet{
					types.Rust: {
						Path:     r.filename,
						LineFrom: lineFrom,
						LineTo:   lineTo,
					},
				},
				Parameters: parameters,
				ParametersRef: map[types.Lang]types.Snippet{
					types.Rust: paramSnippet,
				},
				Result:    result,
				ResultRef: map[types.Lang]types.Snippet{},
			}
			if result != nil {
				tx.ResultRef[types.Rust] = *resultSnippet
			}
			txs = append(txs, tx)
		}
	}

	// TODO: Add implicit Parameters transaction!
	return txs, nil
}

// findParamsResultName extracts the type name for parameters and result.
func (r RustParser) findParamsResultName(text []string, lineIdx int) (paramsName string, resultName string) {
	paramsRegMatch, _ := regexp.Compile(".*types::([a-zA-Z]+)")
	for ; lineIdx < len(text); lineIdx += 1 {
		paramsMatch := paramsRegMatch.FindStringSubmatch(text[lineIdx])
		if len(paramsMatch) > 0 {
			paramsName = paramsMatch[1]
			break
		}
	}

	resultRegMatch, _ := regexp.Compile(".*Result<types::([a-zA-Z]+)")
	for ; lineIdx < len(text); lineIdx += 1 {
		resultMatch := resultRegMatch.FindStringSubmatch(text[lineIdx])
		if len(resultMatch) > 0 {
			resultName = resultMatch[1]
			break
		}
		if strings.Contains(text[lineIdx], "{") {
			break
		}
	}
	return
}

func (r RustParser) findMembers(name string) ([]types.Parameter, *types.Snippet, error) {
	typesFilename := r.getTypesFile()
	text, err := readFile(typesFilename)
	if err != nil {
		return nil, nil, err
	}

	lineIdx := 0
	for ; lineIdx < len(text); lineIdx += 1 {
		if text[lineIdx] == fmt.Sprintf("pub struct %s {", name) {
			break
		}
	}
	if lineIdx == len(text) {
		// No result defined which is fine.
		return nil, nil, nil
	}

	// -1 because of #[derive...] directives.
	_, lineFrom := findComment(text, lineIdx-1, "")
	lineTo, err := findEndBlock(text, lineIdx, "", name)
	if err != nil {
		return nil, nil, err
	}
	snippet := types.Snippet{
		Path:     typesFilename,
		LineFrom: lineFrom,
		LineTo:   lineTo,
	}

	// Find parameters.
	regParamMatch, _ := regexp.Compile("    pub (.*): (.*),")
	params := []types.Parameter{}
	for ; lineIdx < lineTo; lineIdx += 1 {
		paramMatch := regParamMatch.FindStringSubmatch(text[lineIdx])
		if len(paramMatch) > 0 {
			desc, _ := findComment(text, lineIdx, "    ")
			param := types.Parameter{
				Name:        paramMatch[1],
				Type:        paramMatch[2],
				Description: desc,
			}
			params = append(params, param)
		}
	}

	return params, &snippet, nil
}

func (r RustParser) mustFindMembers(name string) ([]types.Parameter, types.Snippet, error) {
	params, snippet, err := r.findMembers(name)
	if err != nil {
		return nil, types.Snippet{}, err
	}
	if snippet == nil {
		return nil, types.Snippet{}, fmt.Errorf("no definition found for %s in %s", name, r.getTypesFile())
	}

	return params, *snippet, nil
}
