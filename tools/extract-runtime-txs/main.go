// extract-runtime-txs extracts runtime transactions from Rust, Go, and TypeScript sources
package main

import (
	"encoding/json"
	"fmt"
	"go/ast"
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
		Use:     scriptName,
		Short:   "Extracts Runtime transactions from formatted Rust, Go and TypeScript code.",
		Long:    "See README.md for details.",
		Example: "./extract-runtime-txs --codebase.path ../.. --markdown",
		Run:     doExtractRuntimeTxs,
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

func markdownParams(params []types.Parameter) string {
	paramsStr := "\n"
	for _, p := range params {
		paramsStr += fmt.Sprintf("- `%s: %s`\n", p.Name, p.Type)
		if p.Description != "" {
			paramsStr += fmt.Sprintf("\n  %s\n\n", p.Description)
		}
	}
	return paramsStr
}

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
		markdown += fmt.Sprintf("### %s (%s) {#%s}\n", tx.FullName(), tx.Type, tx.Module+"-"+strings.ToLower(tx.Name))
		markdown += fmt.Sprintf("%s\n\n", markdownRef(tx.FullName(), tx.Ref, types.Base))
		markdown += fmt.Sprintf("#### Parameters %s\n%s\n", markdownRef(tx.FullName(), tx.ParametersRef, types.Params), markdownParams(tx.Parameters))

		if tx.Result != nil || len(tx.ResultRef) > 0 {
			markdown += fmt.Sprintf("#### Result %s\n%s\n", markdownRef(tx.FullName(), tx.ResultRef, types.Result), markdownParams(tx.Result))
		}

		for l, s := range tx.Ref {
			markdown += fmt.Sprintf("[%s]: %s\n", refAnchor(l, tx.FullName(), types.Base), snippetPath(s))
		}
		for l, s := range tx.ParametersRef {
			markdown += fmt.Sprintf("[%s]: %s\n", refAnchor(l, tx.FullName(), types.Params), snippetPath(s))
		}
		for l, s := range tx.ResultRef {
			markdown += fmt.Sprintf("[%s]: %s\n", refAnchor(l, tx.FullName(), types.Result), snippetPath(s))
		}

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

func doExtractRuntimeTxs(cmd *cobra.Command, args []string) {
	transactions, err := parsers.GenerateInitialTransactions(viper.GetString(CfgCodebasePath) + "/runtime-sdk")
	if err != nil {
		log.Fatal(err)
	}

	parsers.PopulateGoRefs(transactions, viper.GetString(CfgCodebasePath)+"/client-sdk/go")

	if viper.GetBool(CfgMarkdown) {
		printMarkdown(transactions)
	} else {
		printJSON(transactions)
	}

	for _, w := range parsers.RustWarnings {
		fmt.Fprintln(os.Stderr, w)
	}
	for _, w := range parsers.GolangWarnings {
		fmt.Fprintln(os.Stderr, w)
	}
}

// extractValue returns string value of the identifier or literal.
func extractValue(n ast.Expr) string {
	lit, ok := n.(*ast.BasicLit)
	if ok {
		// Strip quotes.
		return lit.Value[1 : len(lit.Value)-1]
	}

	ident, ok := n.(*ast.Ident)
	if !ok || ident.Obj == nil {
		return ""
	}
	decl, ok := ident.Obj.Decl.(*ast.ValueSpec)
	if !ok || len(decl.Values) != 1 {
		return ""
	}
	val, ok := decl.Values[0].(*ast.BasicLit)
	if !ok {
		return ""
	}
	// Strip quotes.
	return val.Value[1 : len(val.Value)-1]
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
