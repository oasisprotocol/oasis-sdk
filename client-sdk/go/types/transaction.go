// TODO: Move this package to the Go client-sdk.
package types

import (
	"fmt"

	"github.com/oasisprotocol/oasis-core/go/common/cbor"
	"github.com/oasisprotocol/oasis-core/go/common/quantity"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature"
)

// TODO: Signature context: oasis-runtime-sdk/tx: v0 for chain H(<consensus-chain-context> || <runtime-id>)

// LatestTransactionVersion is the latest transaction format version.
const LatestTransactionVersion = 1

// UnverifiedTransaction is an unverified transaction.
type UnverifiedTransaction struct {
	_ struct{} `cbor:",toarray"` // nolint

	Body       []byte
	Signatures [][]byte
}

type TransactionSigner struct {
	tx Transaction
	ut UnverifiedTransaction
}

// AppendSign signs the transaction and appends the signature.
//
// The signer must be specified in the AuthInfo.
func (ts *TransactionSigner) AppendSign(signer signature.Signer) error {
	pk := signer.Public()
	index := -1
	for i, si := range ts.tx.AuthInfo.SignerInfo {
		if !si.PublicKey.Equal(pk) {
			continue
		}

		index = i
		break
	}
	if index == -1 {
		return fmt.Errorf("transaction: signer not found in AuthInfo")
	}
	if len(ts.ut.Signatures) == 0 {
		ts.ut.Signatures = make([][]byte, len(ts.tx.AuthInfo.SignerInfo))
	}
	if len(ts.ut.Signatures) != len(ts.tx.AuthInfo.SignerInfo) {
		return fmt.Errorf("transaction: inconsistent number of signature slots")
	}

	sig, err := signer.ContextSign([]byte("TODO CTX"), ts.ut.Body) // XXX: Context.
	if err != nil {
		return fmt.Errorf("transaction: failed to sign transaction: %w", err)
	}
	ts.ut.Signatures[index] = sig
	return nil
}

// UnverifiedTransaction returns the (signed) unverified transaction.
func (ts *TransactionSigner) UnverifiedTransaction() *UnverifiedTransaction {
	return &ts.ut
}

// Transaction is a runtime transaction.
type Transaction struct {
	cbor.Versioned

	Call     Call     `json:"call"`
	AuthInfo AuthInfo `json:"ai"`
}

// ValidateBasic performs basic validation on the transaction.
func (t *Transaction) ValidateBasic() error {
	if t.V != LatestTransactionVersion {
		return fmt.Errorf("transaction: unsupported version")
	}
	if len(t.AuthInfo.SignerInfo) == 0 {
		return fmt.Errorf("transaction: malformed transaction")
	}
	return nil
}

// AppendSignerInfo appends a new transaction signer information to the transaction.
func (t *Transaction) AppendSignerInfo(pk signature.PublicKey, nonce uint64) {
	t.AuthInfo.SignerInfo = append(t.AuthInfo.SignerInfo, SignerInfo{
		PublicKey: PublicKey{pk},
		Nonce:     nonce,
	})
}

func (t *Transaction) PrepareForSigning() *TransactionSigner {
	return &TransactionSigner{
		tx: *t,
		ut: UnverifiedTransaction{
			Body: cbor.Marshal(t),
		},
	}
}

// NewTransaction creates a new unsigned transaction.
func NewTransaction(fee *Fee, method string, body interface{}) *Transaction {
	tx := &Transaction{
		Versioned: cbor.NewVersioned(LatestTransactionVersion),
		Call: Call{
			Method: method,
			Body:   cbor.Marshal(body),
		},
	}
	if fee != nil {
		tx.AuthInfo.Fee = *fee
	} else {
		// Set up a default amount to avoid invalid serialization.
		tx.AuthInfo.Fee.Amount = NewBaseUnits(*quantity.NewFromUint64(0), NativeDenomination)
	}
	return tx
}

// Call is a method call.
type Call struct {
	Method string          `json:"method"`
	Body   cbor.RawMessage `json:"body"`
}

// AuthInfo contains transaction authentication information.
type AuthInfo struct {
	SignerInfo []SignerInfo `json:"si"`
	Fee        Fee          `json:"fee"`
}

// Fee contains the transaction fee information.
type Fee struct {
	Amount BaseUnits `json:"amount"`
	Gas    uint64    `json:"gas"`
}

// SignerInfo contains transaction signer information.
type SignerInfo struct {
	PublicKey PublicKey `json:"pub"`
	Nonce     uint64    `json:"nonce"`
}

// CallResult is the method call result.
type CallResult struct {
	Ok     cbor.RawMessage   `json:"ok,omitempty"`
	Failed *FailedCallResult `json:"fail,omitempty"`
}

// IsSuccess checks whether the call result indicates success.
func (cr *CallResult) IsSuccess() bool {
	return cr.Failed == nil
}

// FailedCallResult is a failed call result.
type FailedCallResult struct {
	Module string `json:"module"`
	Code   uint32 `json:"code"`
}

// Error is a trivial implementation of error.
func (cr FailedCallResult) Error() string {
	return cr.String()
}

// String returns the string representation of a failed call result.
func (cr FailedCallResult) String() string {
	return fmt.Sprintf("module: %s code: %d", cr.Module, cr.Code)
}
