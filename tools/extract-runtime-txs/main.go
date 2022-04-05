// extract-runtime-txs extracts runtime transactions from Rust, Go, and TypeScript sources
package main

import (
	"encoding/json"
	"fmt"
	"io/ioutil"
	"log"
	"os"
	"path/filepath"
	"sort"
	"strings"

	"github.com/oasisprotocol/oasis-sdk/tools/extract-runtime-txs/parsers"
	"github.com/oasisprotocol/oasis-sdk/tools/extract-runtime-txs/types"
	"github.com/spf13/cobra"
	"github.com/spf13/viper"
)

const (
	CfgMarkdown               = "markdown"
	CfgMarkdownTplFile        = "markdown.template.file"
	CfgMarkdownTplPlaceholder = "markdown.template.placeholder"
	CfgCodebasePath           = "codebase.path"
	CfgCodebaseURL            = "codebase.url"
)

var (
	scriptName = filepath.Base(os.Args[0])

	rootCmd = &cobra.Command{
		Use:   scriptName,
		Short: "Extracts Runtime transactions from formatted Rust, Go and TypeScript code.",
		Example: `./extract-runtime-txs \
        --codebase.path ../.. \
        --markdown \
        --markdown.template.file ../../docs/runtime/transactions.md.tpl \
        --codebase.url https://github.com/oasisprotocol/oasis-sdk/tree/master/ \
        > ../../docs/runtime/transactions.md`,
		Run: doExtractRuntimeTxs,
	}
)

// refAnchor returns the reference name.
func refAnchor(l types.Lang, fullName string, t types.RefType) string {
	refTypeStr := ""
	if t != types.Base {
		refTypeStr = fmt.Sprintf("-%s", t)
	}

	return fmt.Sprintf("%s-%s%s", l, fullName, refTypeStr)
}

// markdownRefSrcs generates a sorted list by language of URL sources for the given references.
func markdownRefSrcs(fullName string, refs map[types.Lang]types.Snippet, refType types.RefType) string {
	markdown := ""
	for _, lang := range []types.Lang{types.Rust, types.Go, types.TypeScript} {
		if _, valid := refs[lang]; !valid {
			continue
		}
		markdown += fmt.Sprintf("[%s]: %s\n", refAnchor(lang, fullName, refType), snippetPath(refs[lang]))
	}

	return markdown
}

// markdownRef generates [ Go | Rust | TypeScript ] for the provided snippet.
func markdownRef(fullName string, snippets map[types.Lang]types.Snippet, t types.RefType) string {
	langMarkdown := []string{}
	for _, lang := range []types.Lang{types.Rust, types.Go, types.TypeScript} {
		if _, valid := snippets[lang]; !valid {
			continue
		}
		ref := fmt.Sprintf("[%s][%s]", lang.ToString(), refAnchor(lang, fullName, t))
		langMarkdown = append(langMarkdown, ref)
	}

	return fmt.Sprintf("[%s]", strings.Join(langMarkdown, " | "))
}

// markdownParams generates a markdown list of parameter or results fields of the transaction.
func markdownParams(params []types.Parameter) string {
	paramsStr := ""
	for _, p := range params {
		paramsStr += fmt.Sprintf("- `%s: %s`\n", p.Name, p.Type)
		if p.Description != "" {
			paramsStr += fmt.Sprintf("\n  %s\n", p.Description)
		}
	}
	return paramsStr
}

// snippetPath populates the file name with line numbers and optionally replaces the local filename
// with the github's or other git repository base URL.
func snippetPath(s types.Snippet) string {
	baseDir := viper.GetString(CfgCodebasePath)
	if viper.IsSet(CfgMarkdownTplFile) && !viper.IsSet(CfgCodebaseURL) {
		baseDir = filepath.Dir(viper.GetString(CfgMarkdownTplFile))
	}
	fileURL, _ := filepath.Rel(baseDir, s.Path)
	if viper.IsSet(CfgCodebaseURL) {
		fileURL = viper.GetString(CfgCodebaseURL) + fileURL
	}
	linesStr := ""
	if s.LineFrom != 0 {
		linesStr = fmt.Sprintf("#L%d", s.LineFrom)
		if s.LineTo != s.LineFrom {
			linesStr += fmt.Sprintf("-L%d", s.LineTo)
		}
	}
	return fmt.Sprintf("%s%s", fileURL, linesStr)
}

// sortTxs sorts the given map of transactions by their key and returns an
// ordered list of transactions.
func sortTxs(txs map[string]types.Tx) []types.Tx {
	keys := make([]string, 0, len(txs))
	for k := range txs {
		keys = append(keys, k)
	}
	sort.Strings(keys)

	sortedTxs := []types.Tx{}
	for _, k := range keys {
		sortedTxs = append(sortedTxs, txs[k])
	}

	return sortedTxs
}

func printMarkdown(transactions map[string]types.Tx) {
	markdown := ""
	lastModule := ""
	for _, tx := range sortTxs(transactions) {
		if tx.Module != lastModule {
			markdown += fmt.Sprintf("## %s\n\n", tx.Module)
			lastModule = tx.Module
		}
		markdown += fmt.Sprintf("### %s (%s) {#%s}\n\n", tx.FullName(), tx.Type, tx.Module+"-"+strings.ToLower(tx.Name))
		markdown += fmt.Sprintf("%s\n\n", markdownRef(tx.FullName(), tx.Ref, types.Base))
		markdown += fmt.Sprintf("#### Parameters %s\n\n%s\n", markdownRef(tx.FullName(), tx.ParametersRef, types.Params), markdownParams(tx.Parameters))

		if len(tx.ResultRef) > 0 {
			markdown += fmt.Sprintf("#### Result %s\n\n", markdownRef(tx.FullName(), tx.ResultRef, types.Result))
			if tx.Result != nil {
				markdown += fmt.Sprintf("%s\n", markdownParams(tx.Result))
			}
		}

		markdown += markdownRefSrcs(tx.FullName(), tx.Ref, types.Base)
		markdown += markdownRefSrcs(tx.FullName(), tx.ParametersRef, types.Params)
		markdown += markdownRefSrcs(tx.FullName(), tx.ResultRef, types.Result)

		markdown += "\n"
	}

	if !viper.IsSet(CfgMarkdownTplFile) {
		// Print Markdown only.
		fmt.Print(markdown)
		return
	}

	md, err := ioutil.ReadFile(viper.GetString(CfgMarkdownTplFile))
	if err != nil {
		panic(err)
	}

	mdStr := strings.Replace(string(md), viper.GetString(CfgMarkdownTplPlaceholder)+"\n", markdown, 1)
	fmt.Print(mdStr)
}

func printJSON(txs map[string]types.Tx) {
	data, err := json.Marshal(sortTxs(txs))
	if err != nil {
		panic(err)
	}
	fmt.Printf("%s", data)
}

func printWarnings(parser parsers.Parser) {
	for _, w := range parser.GetWarnings() {
		fmt.Fprintln(os.Stderr, w)
	}
}

func doExtractRuntimeTxs(cmd *cobra.Command, args []string) {
	rustParser := parsers.NewRustParser(viper.GetString(CfgCodebasePath) + "/runtime-sdk")
	transactions, err := rustParser.GenerateInitialTransactions()
	if err != nil {
		log.Fatal(err)
	}
	printWarnings(rustParser)

	prsrs := []parsers.Parser{
		parsers.NewGolangParser(viper.GetString(CfgCodebasePath) + "/client-sdk/go"),
		parsers.NewTypeScriptParser(viper.GetString(CfgCodebasePath) + "/client-sdk/ts-web"),
	}
	for _, p := range prsrs {
		p.PopulateRefs(transactions)
		for _, w := range p.GetWarnings() {
			fmt.Fprintln(os.Stderr, w)
		}
		printWarnings(p)
	}

	if viper.GetBool(CfgMarkdown) {
		printMarkdown(transactions)
	} else {
		printJSON(transactions)
	}
}

func main() {
	rootCmd.Flags().Bool(CfgMarkdown, false, "print metrics in markdown format")
	rootCmd.Flags().String(CfgCodebasePath, "", "path to Go codebase")
	rootCmd.Flags().String(CfgCodebaseURL, "", "show URL to Go files with this base instead of relative path (optional) (e.g. https://github.com/oasisprotocol/oasis-sdk/tree/master/)")
	rootCmd.Flags().String(CfgMarkdownTplFile, "", "path to Markdown template file")
	rootCmd.Flags().String(CfgMarkdownTplPlaceholder, "<!--- OASIS_RUNTIME_TRANSACTIONS -->", "placeholder for Markdown table in the template")
	_ = cobra.MarkFlagRequired(rootCmd.Flags(), CfgCodebasePath)
	_ = viper.BindPFlags(rootCmd.Flags())

	_ = rootCmd.Execute()
}
