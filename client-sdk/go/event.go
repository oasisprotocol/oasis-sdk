package sdk

import (
	"bytes"
	"encoding/binary"
)

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
