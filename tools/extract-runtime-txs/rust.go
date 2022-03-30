package main

import (
	"bufio"
	"os"
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
	file, err := os.Open(r.filename)
	if err != nil {
		return nil, err
	}
	defer file.Close()

	regMatch, _ := regexp.Compile(".*#\\[handler\\((call|query) = \"([a-zA-Z\\.]+)\"\\)\\]")

	txs := []Tx{}
	scanner := bufio.NewScanner(file)
	loc := 0
	for scanner.Scan() {
		loc += 1
		txMatch := regMatch.FindStringSubmatch(scanner.Text())
		if len(txMatch) > 0 {
			txType := Call
			if txMatch[1] == string(Query) {
				txType = Query
			}

			fullNameSplit := strings.Split(txMatch[2], ".")
			tx := Tx{
				Module: fullNameSplit[0],
				Name:   fullNameSplit[1],
				Type:   txType,
				Ref: map[Lang]Snippet{
					Rust: Snippet{
						Path:     r.filename,
						LineFrom: loc+1,
						LineTo:   loc+10,
					},
				},
			}
			txs = append(txs, tx)
		}
	}

	if err := scanner.Err(); err != nil {
		return nil, err
	}

	return txs, nil
}
