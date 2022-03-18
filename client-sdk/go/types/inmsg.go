package types

import (
	"fmt"

	"github.com/oasisprotocol/oasis-core/go/common/cbor"
)

// LatestIncomingMessageVersion is the latest incoming message format version.
const LatestIncomingMessageVersion = 1

type IncomingMessageData struct {
	cbor.Versioned

	// UnverifiedTransaction is an embedded transaction (UnverifiedTransaction).
	// The transaction doesn't need to be from the same account that sent the message.
	UnverifiedTransaction *[]byte `json:"ut"`
}

func (d *IncomingMessageData) ValidateBasic() error {
	if d.V != LatestIncomingMessageVersion {
		return fmt.Errorf("incoming message data: unsupported version")
	}
	return nil
}

func NoopIncomingMessageData() *IncomingMessageData {
	return &IncomingMessageData{
		Versioned:             cbor.NewVersioned(LatestIncomingMessageVersion),
		UnverifiedTransaction: nil,
	}
}
