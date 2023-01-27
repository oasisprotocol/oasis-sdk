package types

import (
	"bytes"
	"encoding/binary"
	"fmt"

	"github.com/oasisprotocol/oasis-core/go/common/crypto/hash"
)

// Event is an event emitted by a runtime in the form of a runtime transaction tag.
//
// Key and value semantics are runtime-dependent.
type Event struct {
	Module string
	Code   uint32
	Value  []byte
	TxHash *hash.Hash
}

// UnmarshalRaw decodes the event from a raw key/value pair.
func (ev *Event) UnmarshalRaw(key, value []byte, txHash *hash.Hash) error {
	if len(key) < 4 {
		return fmt.Errorf("malformed event key")
	}

	ev.Module = string(key[:len(key)-4])
	ev.Code = binary.BigEndian.Uint32(key[len(key)-4:])
	ev.Value = value
	ev.TxHash = txHash
	return nil
}

// Key returns the event key.
func (ev *Event) Key() EventKey {
	return NewEventKey(ev.Module, ev.Code)
}

// EventKey is an event tag key.
type EventKey []byte

// IsEqual compares this event key against another for equality.
func (ek EventKey) IsEqual(other []byte) bool {
	return bytes.Equal(ek[:], other)
}

// NewEventKey generates an event tag key from a module name and event code.
func NewEventKey(module string, code uint32) EventKey {
	key := make([]byte, len(module)+4)
	copy(key[:len(module)], module)
	binary.BigEndian.PutUint32(key[len(module):], code)
	return key
}
