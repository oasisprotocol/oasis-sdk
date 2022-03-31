package main

import (
	"bufio"
	"fmt"
	"os"
	"regexp"
	"strings"
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

// findComment finds the beginning of the comment from the lineIdx upwards and
// extracts the content. If the comment is not found, the initial line is
// returned + 1.
func findComment(text []string, lineIdx int, indent string) (string, int) {
	regMatchComment, _ := regexp.Compile(indent + "//[/] (.*)")
	comment := ""
	for commentLine := lineIdx - 1; commentLine > 0; commentLine -= 1 {
		commentMatch := regMatchComment.FindStringSubmatch(text[commentLine])
		if len(commentMatch) == 0 {
			break
		}
		comment = commentMatch[1] + " " + comment
		lineIdx -= 1
	}
	comment = strings.TrimSpace(comment)

	return comment, lineIdx + 1
}

// findEndBlock finds the end of the block by finding curly parenthesis symbol
// at the specified depth and returns the line number or error, if not found.
func findEndBlock(text []string, lineIdx int, indent string, name string) (int, error) {
	for ; lineIdx < len(text); lineIdx += 1 {
		if text[lineIdx] == (indent + "}") {
			return lineIdx + 1, nil
		}
	}
	return 0, fmt.Errorf("cannot find end of %s block", name)
}
