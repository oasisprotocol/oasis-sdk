package main

import (
	"bufio"
	"os"
)

// readFile reads a complete text file into array of strings, one line per element.
// This comes handy for example when composing function comments which can be multiple lines long
// just before actual function.
func readFile(filename string) ([]string, error) {
	file, err := os.Open(filename)
	if err != nil {
		return nil, err
	}
	defer file.Close()

	scanner := bufio.NewScanner(file)
	text := []string{}
	for scanner.Scan() {
		text = append(text, scanner.Text())
	}

	return text, nil
}
