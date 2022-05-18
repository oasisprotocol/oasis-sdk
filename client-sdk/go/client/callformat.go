package client

import (
	"context"
	"fmt"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/callformat"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

const methodCallDataPublicKey = "core.CallDataPublicKey"

type callDataPublicKeyQueryResponse struct {
	PublicKey types.SignedPublicKey `json:"public_key"`
}

// encodeCall performs call encoding based on the specified call format.
//
// Returns the encoded call and any format-specific metadata needed for decoding the result that
// need to be passed to decodeResult.
func (tb *TransactionBuilder) encodeCall(ctx context.Context, call *types.Call, cf types.CallFormat) (*types.Call, interface{}, error) {
	var cfg callformat.EncodeConfig
	switch cf {
	case types.CallFormatEncryptedX25519DeoxysII:
		// Obtain current calldata X25519 public key.
		var rsp callDataPublicKeyQueryResponse
		if err := tb.rc.Query(ctx, RoundLatest, methodCallDataPublicKey, nil, &rsp); err != nil {
			return nil, nil, fmt.Errorf("callformat: failed to query calldata X25519 public key: %w", err)
		}
		// TODO: In case the node we are connecting to is not trusted, validate the key manager signature.

		cfg.PublicKey = &rsp.PublicKey
	default:
	}

	return callformat.EncodeCall(call, cf, &cfg)
}

// decodeResult performs result decoding based on the specified call format metadata.
func (tb *TransactionBuilder) decodeResult(result *types.CallResult, meta interface{}) (*types.CallResult, error) {
	return callformat.DecodeResult(result, meta)
}
