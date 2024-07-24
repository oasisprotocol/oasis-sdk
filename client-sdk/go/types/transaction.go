// TODO: Move this package to the Go client-sdk.
package types

import (
	"context"
	"encoding/base64"
	"encoding/json"
	"fmt"
	"io"
	"reflect"
	"sync"

	"github.com/oasisprotocol/oasis-core/go/common/cbor"
	"github.com/oasisprotocol/oasis-core/go/common/crypto/hash"
	"github.com/oasisprotocol/oasis-core/go/common/prettyprint"
	"github.com/oasisprotocol/oasis-core/go/common/quantity"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature"
)

var (
	// SignatureContextBase is the transaction signature domain separation context base.
	SignatureContextBase = []byte("oasis-runtime-sdk/tx: v0")

	// registeredMethods maps method name -> class instance.
	registeredMethods sync.Map
)

// LatestTransactionVersion is the latest transaction format version.
const LatestTransactionVersion = 1

// AuthProof is a container for data that authenticates a transaction.
type AuthProof struct {
	// Signature is for signature authentication.
	Signature []byte `json:"signature,omitempty"`
	// Multisig is for multisig authentication.
	Multisig [][]byte `json:"multisig,omitempty"`
	// Module is a flag to use module-controlled decoding. The string is an encoding scheme name
	// that a module must handle. When using this variant, the scheme name must not be empty.
	Module string `json:"module,omitempty"`
}

// UnverifiedTransaction is an unverified transaction.
type UnverifiedTransaction struct {
	_ struct{} `cbor:",toarray"`

	Body       []byte
	AuthProofs []AuthProof
}

// Hash returns the cryptographic hash of the encoded transaction.
func (ut *UnverifiedTransaction) Hash() hash.Hash {
	return hash.NewFrom(ut)
}

// Verify verifies and deserializes the unverified transaction.
func (ut *UnverifiedTransaction) Verify(ctx signature.Context) (*Transaction, error) {
	if len(ut.AuthProofs) == 1 && ut.AuthProofs[0].Module != "" {
		return nil, fmt.Errorf("module-controlled decoding (scheme %q) not supported", ut.AuthProofs[0].Module)
	}

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
	txCtx := ctx.Derive()
	// We'll need at least one signature per proof, so we might as well preallocate that.
	// Could be more though.
	publicKeys := make([]signature.PublicKey, 0, len(ut.AuthProofs))
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

// PrettyPrint writes a pretty-printed representation of the transaction to the given writer.
func (ut *UnverifiedTransaction) PrettyPrint(ctx context.Context, prefix string, w io.Writer) {
	fmt.Fprintf(w, "%sHash: %s\n", prefix, ut.Hash())

	var tx Transaction
	if err := cbor.Unmarshal(ut.Body, &tx); err != nil {
		fmt.Fprintf(w, "%s  <error: %s>\n", prefix, fmt.Errorf("transaction: malformed transaction body: %w", err))
		return
	}

	// Verify and print each signer and signature.
	publicKeys := make([]signature.PublicKey, 0, len(ut.AuthProofs))
	signatures := make([][]byte, 0, len(ut.AuthProofs))
	for i, ap := range ut.AuthProofs {
		pks, sigs, err := tx.AuthInfo.SignerInfo[i].AddressSpec.Batch(ap)
		if err != nil {
			fmt.Fprintf(w, "%s  <error: %s>\n", prefix, fmt.Errorf("transaction: auth proof %d batch: %w", i, err))
		}
		publicKeys = append(publicKeys, pks...)
		signatures = append(signatures, sigs...)
	}

	fmt.Fprintf(w, "%sSigner(s):\n", prefix)
	sigCtx, _ := ctx.Value(signature.ContextKeySigContext).(signature.Context)
	for i, pk := range publicKeys {
		fmt.Fprintf(w, "%s  %d. %s\n", prefix, i+1, pk)
		fmt.Fprintf(w, "%s     (signature: %s)\n", prefix, base64.StdEncoding.EncodeToString(signatures[i]))
		if !pk.Verify(sigCtx.Derive(), ut.Body, signatures[i]) {
			fmt.Fprintf(w, "%s     [INVALID SIGNATURE]\n", prefix)
		}
	}

	fmt.Fprintf(w, "%sContent:\n", prefix)
	tx.PrettyPrint(ctx, prefix+"  ", w)
}

// PrettyType returns a representation of the type that can be used for pretty printing.
func (ut *UnverifiedTransaction) PrettyType() (interface{}, error) {
	return ut, nil
}

type TransactionSigner struct {
	tx Transaction
	ut UnverifiedTransaction
}

func (ts *TransactionSigner) allocateProofs() {
	if len(ts.ut.AuthProofs) == 0 {
		ts.ut.AuthProofs = make([]AuthProof, len(ts.tx.AuthInfo.SignerInfo))

		for i, si := range ts.tx.AuthInfo.SignerInfo {
			if si.AddressSpec.Multisig != nil {
				if len(ts.ut.AuthProofs[i].Multisig) == 0 {
					ts.ut.AuthProofs[i].Multisig = make([][]byte, len(si.AddressSpec.Multisig.Signers))
				}
			}
		}
	}
}

// AppendSign signs the transaction and appends the signature.
//
// The signer must be specified in the AuthInfo.
func (ts *TransactionSigner) AppendSign(ctx signature.Context, signer signature.Signer) error {
	pk := signer.Public()
	isAny := false
	for i, si := range ts.tx.AuthInfo.SignerInfo {
		switch {
		case si.AddressSpec.Signature != nil:
			if !si.AddressSpec.Signature.PublicKey().Equal(pk) {
				continue
			}

			isAny = true
			ts.allocateProofs()
			sig, err := signer.ContextSign(ctx, ts.ut.Body)
			if err != nil {
				return fmt.Errorf("signer info %d: failed to sign transaction: %w", i, err)
			}
			ts.ut.AuthProofs[i].Signature = sig
		case si.AddressSpec.Multisig != nil:
			for j, mss := range si.AddressSpec.Multisig.Signers {
				if !mss.PublicKey.Equal(pk) {
					continue
				}

				isAny = true
				ts.allocateProofs()
				sig, err := signer.ContextSign(ctx, ts.ut.Body)
				if err != nil {
					return fmt.Errorf("signer info %d: failed to sign transaction: %w", i, err)
				}
				ts.ut.AuthProofs[i].Multisig[j] = sig
			}
		default:
			return fmt.Errorf("signer info %d: malformed AddressSpec", i)
		}
	}
	if !isAny {
		return fmt.Errorf("transaction: signer not found in AuthInfo")
	}
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
func (t *Transaction) AppendSignerInfo(addressSpec AddressSpec, nonce uint64) {
	t.AuthInfo.SignerInfo = append(t.AuthInfo.SignerInfo, SignerInfo{
		AddressSpec: addressSpec,
		Nonce:       nonce,
	})
}

// AppendAuthSignature appends a new transaction signer information with a signature address
// specification to the transaction.
func (t *Transaction) AppendAuthSignature(spec SignatureAddressSpec, nonce uint64) {
	t.AppendSignerInfo(AddressSpec{Signature: &spec}, nonce)
}

// AppendAuthMultisig appends a new transaction signer information with a multisig address
// specification to the transaction.
func (t *Transaction) AppendAuthMultisig(config *MultisigConfig, nonce uint64) {
	t.AppendSignerInfo(AddressSpec{Multisig: config}, nonce)
}

func (t *Transaction) PrepareForSigning() *TransactionSigner {
	return &TransactionSigner{
		tx: *t,
		ut: UnverifiedTransaction{
			Body: cbor.Marshal(t),
		},
	}
}

// PrettyPrint writes a pretty-printed representation of the transaction to the given writer.
func (t *Transaction) PrettyPrint(ctx context.Context, prefix string, w io.Writer) {
	pt, err := t.PrettyType()
	if err != nil {
		fmt.Fprintf(w, "%s<error: %s>\n", prefix, err)
	}
	pt.(prettyprint.PrettyPrinter).PrettyPrint(ctx, prefix, w)
}

// PrettyType returns a representation of the type that can be used for pretty printing.
func (t *Transaction) PrettyType() (interface{}, error) {
	var body interface{}
	bodyType := t.Call.Method.BodyType()
	if bodyType != nil {
		// Use the original type.
		body = reflect.New(reflect.TypeOf(bodyType)).Interface()
	} else {
		// Assume a generic map.
		body = &map[string]interface{}{}
	}
	// Try decoding it. If it fails, just take the raw data.
	if err := cbor.Unmarshal(t.Call.Body, body); err != nil {
		body = t.Call.Body
	}

	return &PrettyTransaction{
		Versioned: t.Versioned,
		Call: PrettyCall{
			Format: t.Call.Format,
			Method: t.Call.Method,
			Body:   body,
		},
		AuthInfo: t.AuthInfo,
	}, nil
}

// NewTransaction creates a new unsigned transaction.
func NewTransaction(fee *Fee, method MethodName, body interface{}) *Transaction {
	tx := &Transaction{
		Versioned: cbor.NewVersioned(LatestTransactionVersion),
		Call: Call{
			Format: CallFormatPlain,
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
	// Initialize SignerInfo to avoid it being set to null in case there are no signers specified
	// which is valid in some queries.
	tx.AuthInfo.SignerInfo = []SignerInfo{}
	return tx
}

// NewEncryptedTransaction creates a new unsigned transaction.
func NewEncryptedTransaction(fee *Fee, body interface{}) *Transaction {
	tx := &Transaction{
		Versioned: cbor.NewVersioned(LatestTransactionVersion),
		Call: Call{
			Format: CallFormatEncryptedX25519DeoxysII,
			Method: "",
			Body:   cbor.Marshal(body),
		},
	}
	if fee != nil {
		tx.AuthInfo.Fee = *fee
	} else {
		// Set up a default amount to avoid invalid serialization.
		tx.AuthInfo.Fee.Amount = NewBaseUnits(*quantity.NewFromUint64(0), NativeDenomination)
	}
	// Initialize SignerInfo to avoid it being set to null in case there are no signers specified
	// which is valid in some queries.
	tx.AuthInfo.SignerInfo = []SignerInfo{}
	return tx
}

// CallFormat is the format used for encoding the call (and output) information.
type CallFormat uint8

const (
	// CallFormatPlain is the plain text call format.
	CallFormatPlain = CallFormat(0)
	// CallFormatEncryptedX25519DeoxysII is the encrypted call format using X25519 for key exchange
	// and Deoxys-II for symmetric encryption.
	CallFormatEncryptedX25519DeoxysII = CallFormat(1)
)

// String returns a string representation of the call format.
func (cf CallFormat) String() string {
	switch cf {
	case CallFormatPlain:
		return "plain"
	case CallFormatEncryptedX25519DeoxysII:
		return "encrypted/x25519-deoxysii"
	default:
		return "[unknown]"
	}
}

// Call is a method call.
type Call struct {
	Format   CallFormat      `json:"format,omitempty"`
	Method   MethodName      `json:"method,omitempty"`
	Body     cbor.RawMessage `json:"body"`
	ReadOnly bool            `json:"ro,omitempty"`
}

// AuthInfo contains transaction authentication information.
type AuthInfo struct {
	SignerInfo []SignerInfo `json:"si"`
	Fee        Fee          `json:"fee"`
	NotBefore  *uint64      `json:"not_before,omitempty"`
	NotAfter   *uint64      `json:"not_after,omitempty"`
}

// Fee contains the transaction fee information.
type Fee struct {
	Amount            BaseUnits `json:"amount"`
	Gas               uint64    `json:"gas,omitempty"`
	ConsensusMessages uint32    `json:"consensus_messages,omitempty"`
	Proxy             *FeeProxy `json:"proxy,omitempty"`
}

// GasPrice returns the gas price implied by the amount and gas.
func (f *Fee) GasPrice() *quantity.Quantity {
	if f.Amount.Amount.IsZero() || f.Gas == 0 {
		return quantity.NewQuantity()
	}

	var gasQ quantity.Quantity
	if err := gasQ.FromUint64(f.Gas); err != nil {
		// Should never happen.
		panic(err)
	}

	amt := f.Amount.Amount.Clone()
	if err := amt.Quo(&gasQ); err != nil {
		// Should never happen.
		panic(err)
	}
	return amt
}

// PrettyPrint writes a pretty-printed representation of the transaction to the given writer.
func (f *Fee) PrettyPrint(ctx context.Context, prefix string, w io.Writer) {
	fmt.Fprintf(w, "%sAmount: ", prefix)
	f.Amount.PrettyPrint(ctx, prefix, w)
	fmt.Fprintln(w)

	fmt.Fprintf(w, "%sGas limit: %d\n", prefix, f.Gas)
	fmt.Fprintf(w, "%s(gas price: ", prefix)
	gp := NewBaseUnits(*f.GasPrice(), f.Amount.Denomination)
	gp.PrettyPrint(ctx, prefix, w)
	fmt.Fprintln(w, " per gas unit)")

	if f.Proxy != nil {
		fmt.Fprintf(w, "%sFee proxy:\n", prefix)
		fmt.Fprintf(w, "%s  Module: %s\n", prefix, f.Proxy.Module)
		fmt.Fprintf(w, "%s  ID:     %x\n", prefix, f.Proxy.ID)
	}
}

// PrettyType returns a representation of the type that can be used for pretty printing.
func (f *Fee) PrettyType() (interface{}, error) {
	return f, nil
}

// FeeProxy contains information about a fee proxy.
type FeeProxy struct {
	// Module that will handle the proxy payment.
	Module string `json:"module"`
	// ID is a module-specific identifier that will handle fee payments for the transaction signer.
	ID []byte `json:"id"`
}

// CallerAddress is a caller address.
type CallerAddress struct {
	// Address is an oasis address.
	Address *Address `json:"address,omitempty"`
	// EthAddress is an ethereum address.
	EthAddress *[20]byte `json:"eth_address,omitempty"`
}

// AddressSpec is common information that specifies an address as well as how to authenticate.
type AddressSpec struct {
	// Signature is for signature authentication.
	Signature *SignatureAddressSpec `json:"signature,omitempty"`
	// Multisig is for multisig authentication.
	Multisig *MultisigConfig `json:"multisig,omitempty"`
}

// Address derives the address.
func (as *AddressSpec) Address() (Address, error) {
	switch {
	case as.Signature != nil:
		return NewAddress(*as.Signature), nil
	case as.Multisig != nil:
		return NewAddressFromMultisig(as.Multisig), nil
	default:
		return Address{}, fmt.Errorf("malformed AddressSpec")
	}
}

// Batch checks that the address specification and the authentication proof are acceptable.
// Returns vectors of public keys and signatures for batch verification of included signatures.
func (as *AddressSpec) Batch(ap AuthProof) ([]signature.PublicKey, [][]byte, error) {
	switch {
	case as.Signature != nil && ap.Signature != nil:
		return []signature.PublicKey{as.Signature.PublicKey()}, [][]byte{ap.Signature}, nil
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
	Ok      cbor.RawMessage   `json:"ok,omitempty"`
	Failed  *FailedCallResult `json:"fail,omitempty"`
	Unknown cbor.RawMessage   `json:"unknown,omitempty"`
}

// IsSuccess checks whether the call result indicates success.
func (cr *CallResult) IsSuccess() bool {
	return cr.Failed == nil
}

// IsUnknown checks whether the call result is unknown.
func (cr *CallResult) IsUnknown() bool {
	return cr.Unknown != nil
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

// PrettyTransaction returns a representation of the type that can be used for pretty printing.
type PrettyTransaction struct {
	cbor.Versioned

	Call     PrettyCall `json:"call"`
	AuthInfo AuthInfo   `json:"ai"`
}

// PrettyPrint writes a pretty-printed representation of the transaction to the given writer.
func (ptx *PrettyTransaction) PrettyPrint(ctx context.Context, prefix string, w io.Writer) {
	fmt.Fprintf(w, "%sFormat: %s\n", prefix, ptx.Call.Format)

	if ptx.Call.Method != "" {
		fmt.Fprintf(w, "%sMethod: %s\n", prefix, ptx.Call.Method)
	}

	fmt.Fprintf(w, "%sBody:\n", prefix)
	// If the body type supports pretty printing, use that.
	if pp, ok := ptx.Call.Body.(prettyprint.PrettyPrinter); ok {
		pp.PrettyPrint(ctx, prefix+"  ", w)
	} else {
		// Otherwise, just serialize into JSON and display that.
		data, err := json.MarshalIndent(ptx.Call.Body, prefix+"  ", "  ")
		if err != nil {
			fmt.Fprintf(w, "%s<error: %s>\n", prefix+"  ", err)
		}
		fmt.Fprintf(w, "%s%s\n", prefix+"  ", data)
	}

	fmt.Fprintf(w, "%sAuthorized signer(s):\n", prefix)
	for idx, si := range ptx.AuthInfo.SignerInfo {
		fmt.Fprintf(w, "%s  %d. %s (%s)\n", prefix, idx+1, si.AddressSpec.Signature.PublicKey(), prettyAddressSpecSig(si.AddressSpec.Signature))
		fmt.Fprintf(w, "%s     Nonce: %d\n", prefix, si.Nonce)
	}

	fmt.Fprintf(w, "%sFee:\n", prefix)
	ptx.AuthInfo.Fee.PrettyPrint(ctx, prefix+"  ", w)
}

func prettyAddressSpecSig(spec interface{}) string {
	specMap := map[string]interface{}{}

	specBytes, _ := json.Marshal(spec)
	_ = json.Unmarshal(specBytes, &specMap)

	for k := range specMap {
		return k
	}
	return ""
}

func (ptx *PrettyTransaction) PrettyType() (interface{}, error) {
	return ptx, nil
}

// PrettyCall returns a representation of the type that can be used for pretty printing.
type PrettyCall struct {
	Format CallFormat  `json:"format,omitempty"`
	Method MethodName  `json:"method,omitempty"`
	Body   interface{} `json:"body"`
}

// MethodName is a method name.
type MethodName string

// SanityCheck performs a basic sanity check on the method name.
func (m MethodName) SanityCheck() error {
	if len(m) == 0 {
		return fmt.Errorf("transaction: empty method")
	}

	return nil
}

// BodyType returns the registered body type associated with this method.
func (m MethodName) BodyType() interface{} {
	bodyType, _ := registeredMethods.Load(string(m))
	return bodyType
}

// NewMethodName creates a new method name.
//
// Module and method pair must be unique. If they are not, this method
// will panic.
func NewMethodName(name string, bodyType interface{}) MethodName {
	// Check for duplicate method names.
	if _, isRegistered := registeredMethods.Load(name); isRegistered {
		panic(fmt.Errorf("transaction: method already registered: %s", name))
	}
	registeredMethods.Store(name, bodyType)

	return MethodName(name)
}

// MethodNames returns a map of all registered method names and its class instances.
func MethodNames() map[string]interface{} {
	// Make a copy of method names instead of exposing it directly.
	cp := map[string]interface{}{}
	registeredMethods.Range(func(k, v interface{}) bool {
		cp[fmt.Sprint(k)] = v
		return true
	})

	return cp
}
