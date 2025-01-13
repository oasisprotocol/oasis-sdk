package rofl

import (
	"fmt"

	"gopkg.in/yaml.v3"

	beacon "github.com/oasisprotocol/oasis-core/go/beacon/api"
	"github.com/oasisprotocol/oasis-core/go/common/crypto/signature"
	"github.com/oasisprotocol/oasis-core/go/common/sgx"
	"github.com/oasisprotocol/oasis-core/go/common/sgx/quote"
)

// AppAuthPolicy is the per-application ROFL policy.
type AppAuthPolicy struct {
	// Quotes is a quote policy.
	Quotes quote.Policy `json:"quotes" yaml:"quotes"`
	// Enclaves is the set of allowed enclave identities.
	Enclaves []sgx.EnclaveIdentity `json:"enclaves" yaml:"enclaves"`
	// Endorsements is the set of allowed endorsements.
	Endorsements []AllowedEndorsement `json:"endorsements" yaml:"endorsements"`
	// Fees is the gas fee payment policy.
	Fees FeePolicy `json:"fees" yaml:"fees"`
	// MaxExpiration is the maximum number of future epochs for which one can register.
	MaxExpiration beacon.EpochTime `json:"max_expiration" yaml:"max_expiration"`
}

// AllowedEndorsement is an allowed endorsement policy.
type AllowedEndorsement struct {
	// Any specifies that any node can endorse the enclave.
	Any *struct{} `json:"any,omitempty" yaml:"any,omitempty"`
	// ComputeRole specifies that a compute node for the current runtime can endorse the enclave.
	ComputeRole *struct{} `json:"role_compute,omitempty" yaml:"role_compute,omitempty"`
	// ObserverRole specifies that an observer node for the current runtime can endorse the enclave.
	ObserverRole *struct{} `json:"role_observer,omitempty" yaml:"role_observer,omitempty"`
	// Entity specifies that a registered node from a specific entity can endorse the enclave.
	Entity *signature.PublicKey `json:"entity,omitempty" yaml:"entity,omitempty"`
	// Node specifies that a specific node can endorse the enclave.
	Node *signature.PublicKey `json:"node,omitempty" yaml:"node,omitempty"`
}

// FeePolicy is a gas fee payment policy.
type FeePolicy uint8

const (
	// FeePolicyInstancePays is a fee policy where the application enclave pays the gas fees.
	FeePolicyInstancePays FeePolicy = 1
	// FeePolicyEndorsingNodePays is a fee policy where the endorsing node pays the gas fees.
	FeePolicyEndorsingNodePays FeePolicy = 2

	nameFeePolicyInstancePays      = "instance"
	nameFeePolicyEndorsingNodePays = "endorsing_node"
)

// UnmarshalYAML implements yaml.Unmarshaler.
func (fp *FeePolicy) UnmarshalYAML(value *yaml.Node) error {
	switch value.ShortTag() {
	case "!!str":
		var feePolicyStr string
		if err := value.Decode(&feePolicyStr); err != nil {
			return err
		}

		switch feePolicyStr {
		case nameFeePolicyInstancePays:
			*fp = FeePolicyInstancePays
		case nameFeePolicyEndorsingNodePays:
			*fp = FeePolicyEndorsingNodePays
		default:
			return fmt.Errorf("unsupported fee policy: '%s'", feePolicyStr)
		}
		return nil
	default:
		return fmt.Errorf("unsupported fee policy type")
	}
}

// MarshalYAML implements yaml.Marshaler.
func (fp FeePolicy) MarshalYAML() (interface{}, error) {
	switch fp {
	case FeePolicyInstancePays:
		return nameFeePolicyInstancePays, nil
	case FeePolicyEndorsingNodePays:
		return nameFeePolicyEndorsingNodePays, nil
	default:
		return nil, fmt.Errorf("unsupported fee policy: %d", fp)
	}
}
