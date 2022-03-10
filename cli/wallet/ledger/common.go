package ledger

import (
	"bytes"
	"encoding/binary"
	"fmt"
	"io"
)

func getAdr0008Path(number uint32) []uint32 {
	return []uint32{44, 474, number}
}

func getLegacyPath(number uint32) []uint32 {
	return []uint32{44, 474, 0, 0, number}
}

func getBip44bytes(bip44Path []uint32) ([]byte, error) {
	message := make([]byte, 4*len(bip44Path))
	switch len(bip44Path) {
	case 5:
		// Legacy derivation path.
	case 3:
		// ADR-0008 derivation path.
	default:
		return nil, fmt.Errorf("path should contain either 5 or 3 elements")
	}

	for index, element := range bip44Path {
		pos := index * 4
		value := element | 0x80000000 // Harden all components.
		binary.LittleEndian.PutUint32(message[pos:], value)
	}
	return message, nil
}

func prepareChunks(bip44PathBytes, context, message []byte, chunkSize int) ([][]byte, error) {
	if len(context) > 255 {
		return nil, fmt.Errorf("maximum supported context size is 255 bytes")
	}

	body := append([]byte{byte(len(context))}, context...)
	body = append(body, message...)

	packetCount := 1 + len(body)/chunkSize
	if len(body)%chunkSize > 0 {
		packetCount++
	}

	chunks := make([][]byte, 0, packetCount)
	chunks = append(chunks, bip44PathBytes) // First chunk is path.

	r := bytes.NewReader(body)
readLoop:
	for {
		toAppend := make([]byte, chunkSize)
		n, err := r.Read(toAppend)
		if n > 0 {
			// Note: n == 0 only when EOF.
			chunks = append(chunks, toAppend[:n])
		}
		switch err {
		case nil:
		case io.EOF:
			break readLoop
		default:
			// This can never happen, but handle it.
			return nil, err
		}
	}

	return chunks, nil
}
