package roflmarket

import (
	"context"
	"encoding/hex"
	"fmt"
	"io"

	"github.com/oasisprotocol/oasis-core/go/common/crypto/hash"
	"github.com/oasisprotocol/oasis-core/go/common/crypto/signature"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/modules/rofl"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

type (
	// OfferID is the per-provider offer identifier.
	OfferID [8]byte
	// InstanceID is the per-provider instance identifier.
	InstanceID [8]byte
	// CommandID is the per-instnce command identifier.
	CommandID [8]byte
)

// String returns a string representation of offer ID.
func (id OfferID) String() string {
	return hex.EncodeToString(id[:])
}

// MarshalText encodes an offer ID.
func (id *OfferID) MarshalText() ([]byte, error) {
	return []byte(hex.EncodeToString(id[:])), nil
}

// UnmarshalText decodes a text marshalled offer ID.
func (id *OfferID) UnmarshalText(data []byte) error {
	if hex.DecodedLen(len(data)) != 8 {
		return fmt.Errorf("malformed offer ID")
	}
	_, err := hex.Decode(id[:], data)
	return err
}

// String returns a string representation of instance ID.
func (id InstanceID) String() string {
	return hex.EncodeToString(id[:])
}

// MarshalText encodes an instance ID.
func (id *InstanceID) MarshalText() ([]byte, error) {
	return []byte(hex.EncodeToString(id[:])), nil
}

// UnmarshalText decodes a text marshalled instance ID.
func (id *InstanceID) UnmarshalText(data []byte) error {
	if hex.DecodedLen(len(data)) != 8 {
		return fmt.Errorf("malformed instance ID")
	}
	_, err := hex.Decode(id[:], data)
	return err
}

// String returns a string representation of command ID.
func (id CommandID) String() string {
	return hex.EncodeToString(id[:])
}

// MarshalText encodes a command ID.
func (id *CommandID) MarshalText() ([]byte, error) {
	return []byte(hex.EncodeToString(id[:])), nil
}

// UnmarshalText decodes a text marshalled command ID.
func (id *CommandID) UnmarshalText(data []byte) error {
	if hex.DecodedLen(len(data)) != 8 {
		return fmt.Errorf("malformed command ID")
	}
	_, err := hex.Decode(id[:], data)
	return err
}

// Provider is the provider descriptor.
type Provider struct {
	// Address is the address of the provider.
	Address types.Address `json:"address"`
	// Nodes are the nodes authorized to act on behalf of provider.
	Nodes []signature.PublicKey `json:"nodes"`
	// SchedulerApp is the authorized scheduler app for this provider.
	SchedulerApp rofl.AppID `json:"scheduler_app"`
	// PaymentAddress is the payment address.
	PaymentAddress PaymentAddress `json:"payment_address"`
	// Metadata is arbitrary metadata (key-value pairs) assigned by the provider.
	Metadata map[string]string `json:"metadata"`

	// Stake is the amount staked for provider registration.
	Stake types.BaseUnits `json:"stake"`
	// OffersNextID is the next offer identifier to use.
	OffersNextID OfferID `json:"offers_next_id"`
	// OffersCount is the number of offers.
	OffersCount uint64 `json:"offers_count"`
	// InstancesNextID is the next instance identifier to use.
	InstancesNextID InstanceID `json:"instances_next_id"`
	// InstancesCount is the number of instances.
	InstancesCount uint64 `json:"instances_count"`
	// CreatedAt is the timestamp when the provider was created at.
	CreatedAt uint64 `json:"created_at"`
	// UpdatedAt is the timestamp when the provider was last updated at.
	UpdatedAt uint64 `json:"updated_at"`
}

// Offer is the offer descriptor.
type Offer struct {
	// ID is the unique offer identifier.
	ID OfferID `json:"id"`
	// Resources are the offered resources.
	Resources Resources `json:"resources"`
	// Payment is the payment for this offer.
	Payment Payment `json:"payment"`
	// Capacity is the amount of available instances. Setting this to zero will disallow
	// provisioning of new instances for this offer. Each accepted instance will automatically
	// decrement capacity.
	Capacity uint64 `json:"capacity"`
	// Metadata is arbitrary metadata (key-value pairs) assigned by the provider.
	Metadata map[string]string `json:"metadata"`
}

// Term is the reservation term.
type Term uint8

const (
	TermHour  Term = 1
	TermMonth Term = 2
	TermYear  Term = 3
)

// PaymentAddress is the payment address.
type PaymentAddress struct {
	Native *types.Address `json:"native,omitempty"`
	Eth    *[20]byte      `json:"eth,omitempty"`
}

// Payment is the payment information.
type Payment struct {
	Native      *NativePayment      `json:"native,omitempty"`
	EvmContract *EvmContractPayment `json:"evm,omitempty"`
}

// NativePayment is the native payment information.
type NativePayment struct {
	Denomination types.Denomination      `json:"denomination"`
	Terms        map[Term]types.Quantity `json:"terms"`
}

// EvmContractPayment is the EVM contract-based payment information.
type EvmContractPayment struct {
	Address [20]byte `json:"address"`
	Data    []byte   `json:"data"`
}

// Resources are describe the requested instance resources.
type Resources struct {
	// TEE is the type of TEE hardware.
	TEE TeeType `json:"tee"`
	// Memory is the amount of memory in megabytes.
	Memory uint64 `json:"memory"`
	// CPUCount is the amount of vCPUs.
	CPUCount uint16 `json:"cpus"`
	// Storage is the amount of storage ine megabytes.
	Storage uint64 `json:"storage"`
	// GPU is the optional GPU resource.
	GPU *GPUResource `json:"gpu,omitempty"`
}

// TeeType is the type of TEE hardware.
type TeeType uint8

const (
	TeeTypeSGX TeeType = 1
	TeeTypeTDX TeeType = 2
)

// GPUResources is the GPU resource descriptor.
type GPUResource struct {
	// Model is the optional GPU model.
	Model string `json:"model,omitempty"`
	// Count is the number of GPUs requested.
	Count uint8 `json:"count"`
}

// Instance is the provisioned instance descriptor.
type Instance struct {
	// Provider is the provider address.
	Provider types.Address `json:"provider"`
	// ID is the per-provider unique instance identifier.
	ID InstanceID `json:"id"`
	// Offer is the per-provider offer identifier.
	Offer OfferID `json:"offer"`
	// Status is the status of the instance.
	Status InstanceStatus `json:"status"`
	// Creator is the address of the creator account.
	Creator types.Address `json:"creator"`
	// Admin is the address of the administrator account.
	Admin types.Address `json:"admin"`
	// NodeID is the optional identifier of the node where the instance has been provisioned.
	NodeID *signature.PublicKey `json:"node_id,omitempty"`
	// Metadata is arbitrary metadata (key-value pairs) assigned by the provider's scheduler.
	Metadata map[string]string `json:"metadata"`
	// Resources are the deployed instance resources.
	Resources Resources `json:"resources"`
	// Deployment is the current deployment running on this instance.
	Deployment *Deployment `json:"deployment,omitempty"`
	// CreatedAt is the timestamp when the instance was created at.
	CreatedAt uint64 `json:"created_at"`
	// UpdatedAt is the timestamp when the instance was last updated at.
	UpdatedAt uint64 `json:"updated_at"`

	// PaidFrom is the timestamp from which the instance has been paid for and not yet claimed by
	// the provider.
	PaidFrom uint64 `json:"paid_from"`
	// PaidUntil is the timestamp until which the instance has been paid for.
	PaidUntil uint64 `json:"paid_until"`
	// Payment is the payment information for this instance (copied from offer so that we can handle
	// top-ups and refunds even when the provider changes the original offers).
	Payment Payment `json:"payment"`
	// PaymentAddress is the instance payment address.
	PaymentAddress [20]byte `json:"payment_address"`
	// RefundData is payment method-specific refund information.
	RefundData []byte `json:"refund_data"`

	// CmdNextID is the next command identifier to use.
	CmdNextID CommandID `json:"cmd_next_id"`
	// CmdCount is the number of queued commands.
	CmdCount uint64 `json:"cmd_count"`
}

// InstanceStatus is the status of an instance.
type InstanceStatus uint8

const (
	InstanceStatusCreated   InstanceStatus = 0
	InstanceStatusAccepted  InstanceStatus = 1
	InstanceStatusCancelled InstanceStatus = 2
)

// String returns a string representation of instance status.
func (s InstanceStatus) String() string {
	switch s {
	case InstanceStatusCreated:
		return "created"
	case InstanceStatusAccepted:
		return "accepted"
	case InstanceStatusCancelled:
		return "cancelled"
	default:
		return fmt.Sprintf("[unknown: %d]", s)
	}
}

// Deployment is a descriptor of what is deployed into an instance.
type Deployment struct {
	// AppID is the identifier of the deployed ROFL app.
	AppID rofl.AppID `json:"app_id"`
	// ManifestHash is the ROFL app manifest hash.
	ManifestHash hash.Hash `json:"manifest_hash"`
	// Metadata is arbitrary metadata (key-value pairs) assigned by the deployer.
	Metadata map[string]string `json:"metadata"`
}

// ProviderCreate is the body of roflmarket.ProviderCreate method.
type ProviderCreate struct {
	// Nodes are the nodes authorized to act on behalf of provider.
	Nodes []signature.PublicKey `json:"nodes"`
	// ScheduperApp is the authorized scheduper app for this provider.
	SchedulerApp rofl.AppID `json:"scheduler_app"`
	// PaymentAddress is the payment address.
	PaymentAddress PaymentAddress `json:"payment_address"`
	// Offers is a list of offers available from this provider.
	Offers []Offer `json:"offers"`
	// Metadata is arbitrary metadata (key-value pairs) assigned by the provider.
	Metadata map[string]string `json:"metadata"`
}

// ProviderUpdate is the body of roflmarket.ProviderUpdate method.
type ProviderUpdate struct {
	// Provider is the provider address.
	Provider types.Address `json:"provider"`
	// Nodes are the nodes authorized to act on behalf of provider.
	Nodes []signature.PublicKey `json:"nodes"`
	// ScheduperApp is the authorized scheduper app for this provider.
	SchedulerApp rofl.AppID `json:"scheduler_app"`
	// PaymentAddress is the payment address.
	PaymentAddress PaymentAddress `json:"payment_address"`
	// Metadata is arbitrary metadata (key-value pairs) assigned by the provider.
	Metadata map[string]string `json:"metadata"`
}

// ProviderUpdateOffers is the body of roflmarket.ProviderUpdateOffers method.
type ProviderUpdateOffers struct {
	// Provider is the provider address.
	Provider types.Address `json:"provider"`
	// Add is a list of offers to add.
	Add []Offer `json:"add,omitempty"`
	// Update is a list of offers to update.
	Update []Offer `json:"update,omitempty"`
	// Remove is a list of offer identifiers to remove.
	Remove []OfferID `json:"remove,omitempty"`
}

// ProviderRemove is the body of roflmarket.ProviderRemove method.
type ProviderRemove struct {
	// Provider is the provider address.
	Provider types.Address `json:"provider"`
}

// InstanceCreate is the body of roflmarket.InstanceCreate method.
type InstanceCreate struct {
	// Provider is the provider address.
	Provider types.Address `json:"provider"`
	// Offer is the unique identifier of the provider's offer.
	Offer OfferID `json:"offer"`
	// Admin is the optional administrator address. If not given, the caller becomes the admin.
	Admin *types.Address `json:"admin,omitempty"`
	// Deployment is the optional deployment that should be made once an instance is accepted by the
	// provider. If not specified, it should be done later via `roflmarket.InstanceDeploy`.
	Deployment *Deployment `json:"deployment,omitempty"`
	// Term is the term pricing to use.
	Term Term `json:"term"`
	// TermCount is the number of terms to pay for in advance.
	TermCount uint64 `json:"term_count"`
}

// InstanceTopUp is the body of roflmarket.InstanceTopUp method.
type InstanceTopUp struct {
	// Provider is the provider address.
	Provider types.Address `json:"provider"`
	// ID is the target instance identifier.
	ID InstanceID `json:"id"`
	// Term is the term pricing to use.
	Term Term `json:"term"`
	// TermCount is the number of terms to pay for in advance.
	TermCount uint64 `json:"term_count"`
}

// InstanceCancel is the body of roflmarket.InstanceCancel method.
type InstanceCancel struct {
	// Provider is the provider address.
	Provider types.Address `json:"provider"`
	// ID is the target instance identifier.
	ID InstanceID `json:"id"`
}

// InstanceExecuteCmds is the body of roflmarket.InstanceExecuteCmds method.
type InstanceExecuteCmds struct {
	// Provider is the provider address.
	Provider types.Address `json:"provider"`
	// ID is the target instance identifier.
	ID InstanceID `json:"id"`
	// Cmds are the scheduler-specific commands to execute. Each command is interpreted by the
	// off-chain scheduler and is therefore scheduler-specific.
	//
	// These commands could also be transmitted directly to the provider via an off-chain channel.
	Cmds [][]byte `json:"cmds"`
}

// QueuedCommand is a queued command.
type QueuedCommand struct {
	// ID is the command sequence number.
	ID CommandID `json:"id"`
	// Cmd is the scheduler-specific command to execute.
	Cmd []byte `json:"cmd"`
}

// ProviderQuery is a provider-related query.
type ProviderQuery struct {
	// Provider is the provider address.
	Provider types.Address `json:"provider"`
}

// OfferQuery is a provider-related query.
type OfferQuery struct {
	// Provider is the provider address.
	Provider types.Address `json:"provider"`
	// ID is the offer identifier.
	ID OfferID `json:"id"`
}

// InstanceQuery is a provider-related query.
type InstanceQuery struct {
	// Provider is the provider address.
	Provider types.Address `json:"provider"`
	// ID is the instance identifier.
	ID InstanceID `json:"id"`
}

// StakeThresholds contains staking thresholds.
type StakeThresholds struct {
	ProviderCreate types.BaseUnits `json:"provider_create"`
}

// PrettyPrint writes a pretty-printed representation of the stake thresholds to the given writer.
func (st *StakeThresholds) PrettyPrint(ctx context.Context, prefix string, w io.Writer) {
	fmt.Fprintf(w, "%sStake thresholds:\n", prefix)
	fmt.Fprintf(w, "%s  Provider create: ", prefix)
	st.ProviderCreate.PrettyPrint(ctx, "", w)
	fmt.Fprint(w, "\n")
}

// PrettyType returns a representation of the type that can be used for pretty printing.
func (st *StakeThresholds) PrettyType() (interface{}, error) {
	return st, nil
}

// Parameters are the parameters for the roflmarket module.
type Parameters struct{}

// ModuleName is the roflmarket module name.
const ModuleName = "roflmarket"

const (
	ProviderCreatedEventCode       = 1
	ProviderUpdatedEventCode       = 2
	ProviderRemovedEventCode       = 3
	InstanceCreatedEventCode       = 4
	InstanceUpdatedEventCode       = 5
	InstanceAcceptedEventCode      = 6
	InstanceCancelledEventCode     = 7
	InstanceRemovedEventCode       = 8
	InstanceCommandQueuedEventCode = 9
)

type ProviderCreatedEvent struct {
	Address types.Address `json:"address"`
}

type ProviderUpdatedEvent struct {
	Address types.Address `json:"address"`
}

type ProviderRemovedEvent struct {
	Address types.Address `json:"address"`
}

type InstanceCreatedEvent struct {
	Provider types.Address `json:"provider"`
	ID       InstanceID    `json:"id"`
}

type InstanceUpdatedEvent struct {
	Provider types.Address `json:"provider"`
	ID       InstanceID    `json:"id"`
}

type InstanceAcceptedEvent struct {
	Provider types.Address `json:"provider"`
	ID       InstanceID    `json:"id"`
}

type InstanceCancelledEvent struct {
	Provider types.Address `json:"provider"`
	ID       InstanceID    `json:"id"`
}

type InstanceRemovedEvent struct {
	Provider types.Address `json:"provider"`
	ID       InstanceID    `json:"id"`
}

type InstanceCommandQueuedEvent struct {
	Provider types.Address `json:"provider"`
	ID       InstanceID    `json:"id"`
}

// Event is a rofl module event.
type Event struct {
	ProviderCreated       *ProviderCreatedEvent
	ProviderUpdated       *ProviderUpdatedEvent
	ProviderRemoved       *ProviderRemovedEvent
	InstanceCreated       *InstanceCreatedEvent
	InstanceUpdated       *InstanceUpdatedEvent
	InstanceAccepted      *InstanceAcceptedEvent
	InstanceCancelled     *InstanceCancelledEvent
	InstanceRemoved       *InstanceRemovedEvent
	InstanceCommandQueued *InstanceCommandQueuedEvent
}
