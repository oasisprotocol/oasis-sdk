package rofl

import (
	beacon "github.com/oasisprotocol/oasis-core/go/beacon/api"
	"github.com/oasisprotocol/oasis-core/go/common/crypto/signature"
	"github.com/oasisprotocol/oasis-core/go/common/sgx"
	"github.com/oasisprotocol/oasis-core/go/common/sgx/quote"
)

// AppAuthPolicy is the per-application ROFL policy.
type AppAuthPolicy struct {
	// Quotes is a quote policy.
	Quotes quote.Policy `json:"quotes"`
	// Enclaves is the set of allowed enclave identities.
	Enclaves []sgx.EnclaveIdentity `json:"enclaves"`
	// Endorsements is the set of allowed endorsements.
	Endorsements []AllowedEndorsement `json:"endorsements"`
	// Fees is the gas fee payment policy.
	Fees FeePolicy `json:"fees"`
	// MaxExpiration is the maximum number of future epochs for which one can register.
	MaxExpiration beacon.EpochTime `json:"max_expiration"`
}

// AllowedEndorsement is an allowed endorsement policy.
type AllowedEndorsement struct {
	// Any specifies that any node can endorse the enclave.
	Any *struct{} `json:"any,omitempty"`
	// ComputeRole specifies that a compute node can endorse the enclave.
	ComputeRole *struct{} `json:"role_compute,omitempty"`
	// ObserverRole specifies that an observer node can endorse the enclave.
	ObserverRole *struct{} `json:"role_observer,omitempty"`
	// Entity specifies that a registered node from a specific entity can endorse the enclave.
	Entity *signature.PublicKey `json:"entity,omitempty"`
	// Node specifies that a specific node can endorse the enclave.
	Node *signature.PublicKey `json:"node,omitempty"`
}

// FeePolicy is a gas fee payment policy.
type FeePolicy uint8

const (
	// FeePolicyAppPays is a fee policy where the application enclave pays the gas fees.
	FeePolicyAppPays FeePolicy = 1
	// FeePolicyEndorsingNodePays is a fee policy where the endorsing node pays the gas fees.
	FeePolicyEndorsingNodePays FeePolicy = 2
)
