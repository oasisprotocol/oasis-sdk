// TODO: Move this package to the Go client-sdk.
package types

import (
	"fmt"

	"github.com/oasisprotocol/oasis-core/go/common/cbor"
	"github.com/oasisprotocol/oasis-core/go/common/quantity"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature"
)

// SignatureContextBase is the transaction signature domain separation context base.
var SignatureContextBase = []byte("oasis-runtime-sdk/tx: v0")

// LatestTransactionVersion is the latest transaction format version.
const LatestTransactionVersion = 1

type AuthProof struct {
	Solo     []byte   `json:"solo,omitempty"`
	Multisig [][]byte `json:"multisig,omitempty"`
}

// UnverifiedTransaction is an unverified transaction.
type UnverifiedTransaction struct {
	_ struct{} `cbor:",toarray"`

	Body       []byte
	AuthProofs []AuthProof
}

// Verify verifies and deserializes the unverified transaction.
func (ut *UnverifiedTransaction) Verify(ctx signature.Context) (*Transaction, error) {
	// Deserialize the inner body.
	var tx Transaction
	if err := cbor.Unmarshal(ut.Body, &tx); err != nil {
		return nil, fmt.Errorf("transaction: malformed transaction body: %w", err)
	}
	if err := tx.ValidateBasic(); err != nil {
		return nil, err
	}

	// Basic structure validation.
	if len(ut.AuthProofs) != len(tx.AuthInfo.SignerInfo) {
		return nil, fmt.Errorf("transaction: inconsistent number of auth proofs")
	}

	// Verify all signatures.
	txCtx := ctx.New(SignatureContextBase)
	// We'll need at least one signature per proof. Could be more though.
	publicKeys := make([]PublicKey, 0, len(ut.AuthProofs))
	signatures := make([][]byte, 0, len(ut.AuthProofs))
	for i, ap := range ut.AuthProofs {
		pks, sigs, err := tx.AuthInfo.SignerInfo[i].AddressSpec.Batch(ap)
		if err != nil {
			return nil, fmt.Errorf("transaction: auth proof %d batch: %w", i, err)
		}
		publicKeys = append(publicKeys, pks...)
		signatures = append(signatures, sigs...)
	}
	for i, pk := range publicKeys {
		if !pk.Verify(txCtx, ut.Body, signatures[i]) {
			// If you're looking at the below error message: the numbering doesn't match up with the auth proof indices
			// if the transaction has multisig auth proofs. You have to count up the included signatures inside the
			// multisig auth proofs to find which one (first) failed.
			return nil, fmt.Errorf("transaction: signature %d verification failed", i)
		}
	}

	return &tx, nil
}

type TransactionSigner struct {
	tx Transaction
	ut UnverifiedTransaction
}

// AppendSign signs the transaction and appends the signature.
//
// The signer must be specified in the AuthInfo.
func (ts *TransactionSigner) AppendSign(ctx signature.Context, signer signature.Signer) error {
	pk := signer.Public()
	index := -1
	for i, si := range ts.tx.AuthInfo.SignerInfo {
		if si.AddressSpec.Solo == nil {
			continue
		}
		if !si.AddressSpec.Solo.Equal(pk) {
			continue
		}

		index = i
		break
	}
	if index == -1 {
		return fmt.Errorf("transaction: signer not found in AuthInfo")
	}
	if len(ts.ut.AuthProofs) == 0 {
		ts.ut.AuthProofs = make([]AuthProof, len(ts.tx.AuthInfo.SignerInfo))
	}
	if len(ts.ut.AuthProofs) != len(ts.tx.AuthInfo.SignerInfo) {
		return fmt.Errorf("transaction: inconsistent number of auth proof slots")
	}

	sig, err := signer.ContextSign(ctx.New(SignatureContextBase), ts.ut.Body)
	if err != nil {
		return fmt.Errorf("transaction: failed to sign transaction: %w", err)
	}
	ts.ut.AuthProofs[index].Solo = sig
	return nil
}

// TODO: AppendSign for multisig

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
func (t *Transaction) AppendSignerInfo(addressSpec AddressSpec, nonce uint64) {
	t.AuthInfo.SignerInfo = append(t.AuthInfo.SignerInfo, SignerInfo{
		AddressSpec: addressSpec,
		Nonce:       nonce,
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

type AddressSpec struct {
	Solo     *PublicKey      `json:"solo,omitempty"`
	Multisig *MultisigConfig `json:"multisig,omitempty"`
}

func (as *AddressSpec) Address() (Address, error) {
	switch {
	case as.Solo != nil:
		return NewAddress(as.Solo), nil
	case as.Multisig != nil:
		return NewAddressFromMultisig(as.Multisig), nil
	default:
		return Address{}, fmt.Errorf("malformed AddressSpec")
	}
}

func (as *AddressSpec) Batch(ap AuthProof) ([]PublicKey, [][]byte, error) {
	switch {
	case as.Solo != nil && ap.Solo != nil:
		return []PublicKey{*as.Solo}, [][]byte{ap.Solo}, nil
	case as.Multisig != nil && ap.Multisig != nil:
		return as.Multisig.Batch(ap.Multisig)
	default:
		return nil, nil, fmt.Errorf("malformed AddressSpec and AuthProof pair")
	}
}

// SignerInfo contains transaction signer information.
type SignerInfo struct {
	AddressSpec AddressSpec `json:"address_spec"`
	Nonce       uint64      `json:"nonce"`
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
	Module  string `json:"module"`
	Code    uint32 `json:"code"`
	Message string `json:"message,omitempty"`
}

// Error is a trivial implementation of error.
func (cr FailedCallResult) Error() string {
	return cr.String()
}

// String returns the string representation of a failed call result.
func (cr FailedCallResult) String() string {
	return fmt.Sprintf("module: %s code: %d message: %s", cr.Module, cr.Code, cr.Message)
}
