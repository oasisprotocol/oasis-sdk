package client

import (
	"context"
	"crypto/rand"
	"encoding/base64"
	"fmt"

	"github.com/oasisprotocol/deoxysii"
	"github.com/oasisprotocol/oasis-core/go/common/cbor"
	mrae "github.com/oasisprotocol/oasis-core/go/common/crypto/mrae/api"
	mraeDeoxysii "github.com/oasisprotocol/oasis-core/go/common/crypto/mrae/deoxysii"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

const methodCallDataPublicKey = "core.CallDataPublicKey"

type callDataPublicKeyQueryResponse struct {
	PublicKey types.SignedPublicKey `json:"public_key"`
}

type metaEncryptedX25519DeoxysII struct {
	// sk is the ephemeral secret key for X25519.
	sk *[32]byte
	// pk is the current calldata X25519 public key.
	pk *[32]byte
}

// encodeCall performs call encoding based on the specified call format.
//
// Returns the encoded call and any format-specific metadata needed for decoding the result that
// need to be passed to decodeResult.
func (tb *TransactionBuilder) encodeCall(ctx context.Context, call *types.Call) (*types.Call, interface{}, error) {
	switch call.Format {
	case types.CallFormatPlain:
		// In case of the plain-text data format, we simply pass on the call unchanged.
		return call, nil, nil
	case types.CallFormatEncryptedX25519DeoxysII:
		// Generate ephemeral X25519 key pair.
		pk, sk, err := mrae.GenerateKeyPair(rand.Reader)
		if err != nil {
			return nil, nil, fmt.Errorf("callformat: failed to generate ephemeral X25519 key pair: %w", err)
		}
		// Generate random nonce.
		var nonce [deoxysii.NonceSize]byte
		if _, err := rand.Read(nonce[:]); err != nil {
			return nil, nil, fmt.Errorf("callformat: failed to generate random nonce: %w", err)
		}

		// Obtain current calldata X25519 public key.
		var rsp callDataPublicKeyQueryResponse
		if err := tb.rc.Query(ctx, RoundLatest, methodCallDataPublicKey, nil, &rsp); err != nil {
			return nil, nil, fmt.Errorf("callformat: failed to query calldata X25519 public key: %w", err)
		}
		// TODO: In case the node we are connecting to is not trusted, validate the key manager signature.

		// Seal serialized plain call.
		rawCall := cbor.Marshal(call)
		sealedCall := mraeDeoxysii.Box.Seal(nil, nonce[:], rawCall, nil, &rsp.PublicKey.PublicKey, sk)

		encoded := &types.Call{
			Format: call.Format,
			Method: "",
			Body: cbor.Marshal(&types.CallEnvelopeX25519DeoxysII{
				Pk:    *pk,
				Nonce: nonce,
				Data:  sealedCall,
			}),
		}
		meta := &metaEncryptedX25519DeoxysII{
			sk: sk,
			pk: &rsp.PublicKey.PublicKey,
		}
		return encoded, meta, nil
	default:
		return nil, nil, fmt.Errorf("callformat: unsupported call format")
	}
}

// decodeResult performs result decoding based on the specified call format metadata.
func (tb *TransactionBuilder) decodeResult(result *types.CallResult, meta interface{}) (*types.CallResult, error) {
	switch m := meta.(type) {
	case nil:
		// In case of plain-text data format, we simply pass on the result unchanged.
		return result, nil
	case *metaEncryptedX25519DeoxysII:
		// Make sure the result makes sense in this context.
		switch {
		case result.IsUnknown():
		case result.IsSuccess():
			// Unexpected as a successful result shouldn't be plain.
			return nil, fmt.Errorf("callformat: unexpected result: %s", base64.StdEncoding.EncodeToString(result.Ok))
		default:
			// Submission could fail before call format processing so the result would be plain.
			return nil, result.Failed
		}

		var envelope types.ResultEnvelopeX25519DeoxysII
		if err := cbor.Unmarshal(result.Unknown, &envelope); err != nil {
			return nil, fmt.Errorf("callformat: malformed result envelope: %w", err)
		}

		// Open sealed envelope.
		var (
			pt  []byte
			err error
		)
		if pt, err = mraeDeoxysii.Box.Open(nil, envelope.Nonce[:], envelope.Data, nil, m.pk, m.sk); err != nil {
			return nil, fmt.Errorf("callformat: failed to open result envelope: %w", err)
		}

		var output types.CallResult
		if err = cbor.Unmarshal(pt, &output); err != nil {
			return nil, fmt.Errorf("callformat: malformed result: %w", err)
		}
		return &output, nil
	default:
		return nil, fmt.Errorf("callformat: unsupported call format")
	}
}
