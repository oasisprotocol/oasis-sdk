package main

import (
	"encoding/binary"
	"fmt"
	"strconv"
	"strings"
	"time"

	"github.com/spf13/cobra"

	"github.com/oasisprotocol/oasis-core/go/common/sgx"
	"github.com/oasisprotocol/oasis-core/go/common/sgx/sigstruct"
	"github.com/oasisprotocol/oasis-core/go/runtime/bundle"
)

// constructSigstruct constructs SIGSTRUCTS from provided arguments.
func constructSigstruct(bnd *bundle.Bundle) *sigstruct.Sigstruct {
	// Load SIGSTRUCT fields.
	var date time.Time
	switch {
	case dateStr == "":
		date = time.Now()
	default:
		var err error
		date, err = parseDate(dateStr)
		if err != nil {
			cobra.CheckErr(fmt.Errorf("failed to parse date: %w", err))
		}
	}

	miscSelect, miscSelectMask, err := parseNumNum(miscelectMiscmask)
	if err != nil {
		cobra.CheckErr(fmt.Errorf("failed to parse miscselect: %w", err))
	}

	xfrm, xfrmMask, err := parseNumNum64(xfrm)
	if err != nil {
		cobra.CheckErr(fmt.Errorf("failed to parse xfrm: %w", err))
	}

	attributes, attributesMask, err := parseNumNum64(attributesAttributemask)
	if err != nil {
		cobra.CheckErr(fmt.Errorf("failed to parse attributes: %w", err))
	}
	attributesMask = ^attributesMask
	if bit32 {
		attributes &= ^uint64(sgx.AttributeMode64Bit)
		attributesMask |= uint64(sgx.AttributeMode64Bit)
	}
	if debug {
		attributes |= uint64(sgx.AttributeDebug)
		attributesMask &= ^uint64(sgx.AttributeDebug)
	}

	mrEnclave, err := bnd.MrEnclave()
	if err != nil {
		cobra.CheckErr(fmt.Errorf("failed to get MRENCLAVE from bundle: %w", err))
	}

	return sigstruct.New(
		sigstruct.WithBuildDate(date),
		sigstruct.WithSwDefined(uint32toArray(swdefined)),
		sigstruct.WithISVProdID(isvprodid),
		sigstruct.WithISVSVN(isvsvn),

		sigstruct.WithMiscSelect(miscSelect),
		sigstruct.WithMiscSelectMask(^miscSelectMask),

		sigstruct.WithAttributes(sgx.Attributes{
			Flags: sgx.AttributesFlags(attributes),
			Xfrm:  xfrm,
		}),
		sigstruct.WithAttributesMask([2]uint64{
			attributesMask,
			^xfrmMask,
		}),

		sigstruct.WithEnclaveHash(*mrEnclave),
	)
}

func parseDate(s string) (time.Time, error) {
	return time.Parse("20060102", s)
}

func uint32toArray(v uint32) [4]byte {
	slice := make([]byte, 4)
	binary.LittleEndian.PutUint32(slice, swdefined)
	arr := (*[4]byte)(slice)
	return *arr
}

func parseNumNum(s string) (uint32, uint32, error) {
	splits := strings.SplitN(s, "/", 2)
	n1, err := strconv.ParseUint(splits[0], 0, 32)
	if err != nil {
		return 0, 0, err
	}
	n2, err := strconv.ParseUint(splits[1], 0, 32)
	if err != nil {
		return 0, 0, err
	}
	return uint32(n1), uint32(n2), nil
}

func parseNumNum64(s string) (uint64, uint64, error) {
	splits := strings.SplitN(s, "/", 2)
	n1, err := strconv.ParseUint(splits[0], 0, 64)
	if err != nil {
		return 0, 0, err
	}
	n2, err := strconv.ParseUint(splits[1], 0, 64)
	if err != nil {
		return 0, 0, err
	}
	return n1, n2, nil
}
