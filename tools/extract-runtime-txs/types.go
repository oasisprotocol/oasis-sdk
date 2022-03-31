package main

const (
	Call  TxType = "call"
	Query TxType = "query"

	Rust       Lang = "rust"
	Go         Lang = "go"
	TypeScript Lang = "typescript"

	Base   RefType = "base"
	Params RefType = "params"
	Result RefType = "result"
)

// TxType is a transaction type.
type TxType string

// Lang is a programming language.
type Lang string

// ToString returns a human-readable name of the programming language.
func (l Lang) ToString() string {
	switch l {
	case Rust:
		return "Rust"
	case Go:
		return "Go"
	case TypeScript:
		return "TypeScript"
	}
	return ""
}

type Parameter struct {
	// Name is a name of the member inside the source code.
	Name string `json:"name"`

	// Type is a data type of the field.
	Type string `json:"type"`

	// Description is a comment of the member inside the source code.
	Description string `json:"description"`
}

// RefType stores a programming language anchor type.
type RefType string

// Snippet holds the position of some snippet inside a source file.
type Snippet struct {
	// Path is a source file of the snippet.
	Path string `json:"path"`

	// LineFrom is the beginning of a snippet inside a file. 0, if not defined.
	LineFrom int `json:"line_from"`

	// LineTo is the final line of the snippet inside a file. 0, if not defined.
	LineTo int `json:"line_to"`
}

// Tx corresponds to a single transaction entry.
type Tx struct {
	Module        string           `json:"module"`
	Name          string           `json:"name"`
	Comment       string           `json:"comment"`
	Type          TxType           `json:"type"`
	Ref           map[Lang]Snippet `json:"ref"`
	Parameters    []Parameter      `json:"parameters"`
	ParametersRef map[Lang]Snippet `json:"parameters_ref"`
	Result        *Parameter       `json:"result"`
	ResultRef     map[Lang]Snippet `json:"result_ref"`
}

// FullNama returns the module + transaction name.
func (t Tx) FullName() string {
	return t.Module + "." + t.Name
}
