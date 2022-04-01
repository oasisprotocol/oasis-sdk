package ledger

import (
	"fmt"

	coreSignature "github.com/oasisprotocol/oasis-core/go/common/crypto/signature"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/crypto/signature/ed25519"
)

type ledgerCoreSigner struct {
	path []uint32
	pk   coreSignature.PublicKey
	dev  *ledgerDevice
}

func (ls *ledgerCoreSigner) Public() coreSignature.PublicKey {
	return ls.pk
}

func (ls *ledgerCoreSigner) ContextSign(context coreSignature.Context, message []byte) ([]byte, error) {
	preparedContext, err := coreSignature.PrepareSignerContext(context)
	if err != nil {
		return nil, fmt.Errorf("ledger: failed to prepare signing context: %w", err)
	}

	signature, err := ls.dev.SignEd25519(ls.path, preparedContext, message)
	if err != nil {
		return nil, fmt.Errorf("ledger: failed to sign message: %w", err)
	}
	return signature, nil
}

func (ls *ledgerCoreSigner) String() string {
	return fmt.Sprintf("[ledger signer: %s]", ls.pk)
}

func (ls *ledgerCoreSigner) Reset() {
	_ = ls.dev.Close()
}

type ledgerSigner struct {
	pk  ed25519.PublicKey
	dev *ledgerDevice
}

func (ls *ledgerSigner) Public() signature.PublicKey {
	return ls.pk
}

func (ls *ledgerSigner) ContextSign(context, message []byte) ([]byte, error) {
	return nil, fmt.Errorf("ledger: signing paratime transactions not supported")
}

func (ls *ledgerSigner) Sign(message []byte) ([]byte, error) {
	return nil, fmt.Errorf("ledger: signing paratime transactions not supported")
}

func (ls *ledgerSigner) String() string {
	return fmt.Sprintf("[ledger signer: %s]", ls.pk)
}

func (ls *ledgerSigner) Reset() {
	_ = ls.dev.Close()
}
