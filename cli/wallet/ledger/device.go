package ledger

import (
	"fmt"
	"time"

	coreSignature "github.com/oasisprotocol/oasis-core/go/common/crypto/signature"
	staking "github.com/oasisprotocol/oasis-core/go/staking/api"
	ledger_go "github.com/zondax/ledger-go"
)

// NOTE: Some of this is lifted from https://github.com/oasisprotocol/oasis-core-ledger but updated
//       to conform to the ADR 0008 derivation scheme.

const (
	userMessageChunkSize = 250

	claConsumer = 0x05

	insGetVersion     = 0
	insGetAddrEd25519 = 1
	insSignEd25519    = 2

	payloadChunkInit = 0
	payloadChunkAdd  = 1
	payloadChunkLast = 2

	errMsgInvalidParameters = "[APDU_CODE_BAD_KEY_HANDLE] The parameters in the data field are incorrect"
	errMsgInvalidated       = "[APDU_CODE_DATA_INVALID] Referenced data reversibly blocked (invalidated)"
	errMsgRejected          = "[APDU_CODE_COMMAND_NOT_ALLOWED] Sign request rejected"
)

type VersionInfo struct {
	Major uint8
	Minor uint8
	Patch uint8
}

type ledgerDevice struct {
	raw ledger_go.LedgerDevice
}

func (ld *ledgerDevice) Close() error {
	return ld.raw.Close()
}

// GetVersion returns the current version of the Oasis user app.
func (ld *ledgerDevice) GetVersion() (*VersionInfo, error) {
	message := []byte{claConsumer, insGetVersion, 0, 0, 0}
	response, err := ld.raw.Exchange(message)
	if err != nil {
		return nil, fmt.Errorf("ledger: failed GetVersion request: %w", err)
	}

	if len(response) < 4 {
		return nil, fmt.Errorf("ledger: truncated GetVersion response")
	}

	return &VersionInfo{
		Major: response[1],
		Minor: response[2],
		Patch: response[3],
	}, nil
}

// GetPublicKeyEd25519 returns the Ed25519 public key associated with the given derivation path.
// If the requireConfirmation flag is set, this will require confirmation from the user.
func (ld *ledgerDevice) GetPublicKeyEd25519(bip44Path []uint32, requireConfirmation bool) ([]byte, error) {
	pathBytes, err := getBip44bytes(bip44Path)
	if err != nil {
		return nil, fmt.Errorf("ledger: failed to get BIP44 bytes: %w", err)
	}

	p1 := byte(0)
	if requireConfirmation {
		p1 = byte(1)
	}

	// Prepare message
	header := []byte{claConsumer, insGetAddrEd25519, p1, 0, 0}
	message := append([]byte{}, header...)
	message = append(message, pathBytes...)
	message[4] = byte(len(message) - len(header)) // update length

	response, err := ld.raw.Exchange(message)
	if err != nil {
		return nil, fmt.Errorf("ledger: failed to request public key: %w", err)
	}
	if len(response) < 39 {
		return nil, fmt.Errorf("ledger: truncated GetAddrEd25519 response")
	}

	rawPubkey := response[0:32]
	rawAddr := string(response[32:])

	var pubkey coreSignature.PublicKey
	if err = pubkey.UnmarshalBinary(rawPubkey); err != nil {
		return nil, fmt.Errorf("ledger: device returned malformed public key: %w", err)
	}

	var addrFromDevice staking.Address
	if err = addrFromDevice.UnmarshalText([]byte(rawAddr)); err != nil {
		return nil, fmt.Errorf("ledger: device returned malformed account address: %w", err)
	}
	addrFromPubkey := staking.NewAddress(pubkey)
	if !addrFromDevice.Equal(addrFromPubkey) {
		return nil, fmt.Errorf(
			"ledger: account address computed on device (%s) doesn't match internally computed account address (%s)",
			addrFromDevice,
			addrFromPubkey,
		)
	}

	return rawPubkey, nil
}

// SignEd25519 asks the device to sign the given domain-separated message with the key derived from
// the given derivation path.
func (ld *ledgerDevice) SignEd25519(bip44Path []uint32, context, message []byte) ([]byte, error) {
	pathBytes, err := getBip44bytes(bip44Path)
	if err != nil {
		return nil, fmt.Errorf("ledger: failed to get BIP44 bytes: %w", err)
	}

	chunks, err := prepareChunks(pathBytes, context, message, userMessageChunkSize)
	if err != nil {
		return nil, fmt.Errorf("ledger: failed to prepare chunks: %w", err)
	}

	var finalResponse []byte
	for idx, chunk := range chunks {
		payloadLen := byte(len(chunk))

		var payloadDesc byte
		switch idx {
		case 0:
			payloadDesc = payloadChunkInit
		case len(chunks) - 1:
			payloadDesc = payloadChunkLast
		default:
			payloadDesc = payloadChunkAdd
		}

		message := []byte{claConsumer, insSignEd25519, payloadDesc, 0, payloadLen}
		message = append(message, chunk...)

		response, err := ld.raw.Exchange(message)
		if err != nil {
			switch err.Error() {
			case errMsgInvalidParameters, errMsgInvalidated:
				return nil, fmt.Errorf("ledger: failed to sign: %s", string(response))
			case errMsgRejected:
				return nil, fmt.Errorf("ledger: signing request rejected by user")
			}
			return nil, fmt.Errorf("ledger: failed to sign: %w", err)
		}

		finalResponse = response
	}

	// XXX: Work-around for Oasis App issue of currently not being capable of
	// signing two transactions immediately one after another:
	// https://github.com/Zondax/ledger-oasis/issues/68.
	time.Sleep(100 * time.Millisecond)

	return finalResponse, nil
}

// connectToDevice connects to the first connected Ledger device.
func connectToDevice() (*ledgerDevice, error) {
	ledgerAdmin := ledger_go.NewLedgerAdmin()

	// TODO: Support multiple devices.
	numDevices := ledgerAdmin.CountDevices()
	switch {
	case numDevices == 0:
		return nil, fmt.Errorf("ledger: no devices connected")
	case numDevices > 1:
		return nil, fmt.Errorf("ledger: multiple devices not supported")
	default:
	}

	raw, err := ledgerAdmin.Connect(0)
	if err != nil {
		return nil, fmt.Errorf("ledger: failed to connect to device: %w", err)
	}

	return &ledgerDevice{raw}, nil
}
