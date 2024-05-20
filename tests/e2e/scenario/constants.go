package scenario

import (
	"github.com/oasisprotocol/oasis-core/go/common"
	staking "github.com/oasisprotocol/oasis-core/go/staking/api"
)

var (
	// RuntimeID is the identifier of the compute runtime used in E2E tests.
	RuntimeID common.Namespace
	_         = RuntimeID.UnmarshalHex("8000000000000000000000000000000000000000000000000000000000000000")
	// RuntimeAddress is the address of the compute runtime used in E2E tests.
	RuntimeAddress = staking.NewRuntimeAddress(RuntimeID)

	// KeymanagerID is the identifier of the keymanager runtime used in E2E tests.
	KeymanagerID common.Namespace
	_            = KeymanagerID.UnmarshalHex("c000000000000000ffffffffffffffffffffffffffffffffffffffffffffffff")
)
