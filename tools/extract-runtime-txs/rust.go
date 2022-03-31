package main

import (
	"fmt"
	"regexp"
	"strings"
)

type RustParser struct {
	filename string
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
	for line := 0; line < len(text); line += 1 {
		txMatch := regMatch.FindStringSubmatch(text[line])
		if len(txMatch) > 0 {
			lineFrom := line + 1

			// Check, if the function has a comment and include it.
			regMatchComment, _ := regexp.Compile(txMatch[1] + "/// (.*)")
			comment := ""
			for commentLine := line - 1; commentLine > 0; commentLine -= 1 {
				commentMatch := regMatchComment.FindStringSubmatch(text[commentLine])
				if len(commentMatch) == 0 {
					break
				}
				comment = commentMatch[1] + " " + comment
				lineFrom -= 1
			}
			comment = strings.TrimSpace(comment)

			txType := Call
			if txMatch[2] == string(Query) {
				txType = Query
			}

			fullNameSplit := strings.Split(txMatch[3], ".")

			// Find the end of the function by finding curly parenthesis symbol at the right depth.
			for ; line < len(text); line += 1 {
				if text[line] == (txMatch[1] + "}") {
					break
				}
			}
			if line == len(text) {
				return nil, fmt.Errorf("cannot find end of function %s", txMatch[3])
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
						LineTo:   line + 1,
					},
				},
			}
			txs = append(txs, tx)
		}
	}

	return txs, nil
}
