package main

import (
	"fmt"
	"path/filepath"
	"regexp"
	"strings"
)

type RustParser struct {
	filename string
}

func (r RustParser) getTypesFile() string {
	return filepath.Join(filepath.Dir(r.filename), "types.rs")
}

// FindTransactions scans the given rust source file and looks for the
// #[handler(call = "...")] or #[handler(query = "...")] pattern. For each
// matching pattern, a new Tx is added to the result set.
func (r RustParser) FindTransactions() ([]Tx, error) {
	text, err := readFile(r.filename)
	if err != nil {
		return nil, err
	}

	// some spaces + #[handler(txtype = "txfullname")]
	regMatch, _ := regexp.Compile("([ ]*)#\\[handler\\((call|query) = \"([a-zA-Z\\.]+)\"\\)\\]")

	txs := []Tx{}
	for lineIdx := 0; lineIdx < len(text); lineIdx += 1 {
		txMatch := regMatch.FindStringSubmatch(text[lineIdx])
		if len(txMatch) > 0 {
			// Check, if the function has a comment and include it.
			comment, lineFrom := findComment(text, lineIdx, txMatch[1])
			paramsName, resultName := r.findParamsResultName(text, lineIdx)

			txType := Call
			if txMatch[2] == string(Query) {
				txType = Query
			}

			fullNameSplit := strings.Split(txMatch[3], ".")

			lineTo, err := findEndBlock(text, lineIdx+1, txMatch[1], txMatch[3])
			if err != nil {
				return nil, err
			}

			parameters, paramSnippet, err := r.mustFindParameters(paramsName)
			if err != nil {
				return nil, err
			}

			var result []Parameter
			var resultSnippet *Snippet
			if resultName != "" {
				result, resultSnippet, err = r.findParameters(resultName)
				if err != nil {
					return nil, err
				}
			}
			tx := Tx{
				Module:  fullNameSplit[0],
				Name:    fullNameSplit[1],
				Comment: comment,
				Type:    txType,
				Ref: map[Lang]Snippet{
					Rust: {
						Path:     r.filename,
						LineFrom: lineFrom,
						LineTo:   lineTo,
					},
				},
				Parameters: parameters,
				ParametersRef: map[Lang]Snippet{
					Rust: paramSnippet,
				},
				Result: result,
			}
			if result != nil {
				tx.ResultRef = map[Lang]Snippet{
					Rust: *resultSnippet,
				}
			}
			txs = append(txs, tx)
		}
	}

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

func (r RustParser) findParameters(name string) ([]Parameter, *Snippet, error) {
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
	snippet := Snippet{
		Path:     typesFilename,
		LineFrom: lineFrom,
		LineTo:   lineTo,
	}

	// Find parameters.
	regParamMatch, _ := regexp.Compile("    pub (.*): (.*),")
	params := []Parameter{}
	for ; lineIdx < lineTo; lineIdx += 1 {
		paramMatch := regParamMatch.FindStringSubmatch(text[lineIdx])
		if len(paramMatch) > 0 {
			desc, _ := findComment(text, lineIdx, "    ")
			param := Parameter{
				Name:        paramMatch[1],
				Type:        paramMatch[2],
				Description: desc,
			}
			params = append(params, param)
		}
	}

	return params, &snippet, nil
}

func (r RustParser) mustFindParameters(name string) ([]Parameter, Snippet, error) {
	params, snippet, err := r.findParameters(name)
	if err != nil {
		return nil, Snippet{}, err
	}
	if snippet == nil {
		return nil, Snippet{}, fmt.Errorf("no parameters definition found for %s in %s", name, r.getTypesFile())
	}

	return params, *snippet, nil
}
