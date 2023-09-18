package consensusaccounts

import (
	"context"
	"fmt"
	"io"

	beacon "github.com/oasisprotocol/oasis-core/go/beacon/api"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

// Deposit are the arguments for consensus.Deposit method.
type Deposit struct {
	To     *types.Address  `json:"to,omitempty"`
	Amount types.BaseUnits `json:"amount"`
}

// PrettyPrint writes a pretty-printed representation of the transaction to the given writer.
func (f *Deposit) PrettyPrint(ctx context.Context, prefix string, w io.Writer) {
	types.PrettyPrintToAmount(ctx, prefix, w, f.To, f.Amount)
}

// PrettyType returns a representation of the type that can be used for pretty printing.
func (f *Deposit) PrettyType() (interface{}, error) {
	return f, nil
}

// Withdraw are the arguments for consensus.Withdraw method.
type Withdraw struct {
	To     *types.Address  `json:"to,omitempty"`
	Amount types.BaseUnits `json:"amount"`
}

// PrettyPrint writes a pretty-printed representation of the transaction to the given writer.
func (f *Withdraw) PrettyPrint(ctx context.Context, prefix string, w io.Writer) {
	types.PrettyPrintToAmount(ctx, prefix, w, f.To, f.Amount)
}

// PrettyType returns a representation of the type that can be used for pretty printing.
func (f *Withdraw) PrettyType() (interface{}, error) {
	return f, nil
}

// Delegate are the arguments for consensus.Delegate method.
type Delegate struct {
	To     types.Address   `json:"to"`
	Amount types.BaseUnits `json:"amount"`
}

// PrettyPrint writes a pretty-printed representation of the transaction to the given writer.
func (d *Delegate) PrettyPrint(ctx context.Context, prefix string, w io.Writer) {
	types.PrettyPrintToAmount(ctx, prefix, w, &d.To, d.Amount)
}

// PrettyType returns a representation of the type that can be used for pretty printing.
func (d *Delegate) PrettyType() (interface{}, error) {
	return d, nil
}

// Undelegate are the arguments for consensus.Undelegate method.
type Undelegate struct {
	From   types.Address  `json:"from"`
	Shares types.Quantity `json:"shares"`
}

// PrettyPrint writes a pretty-printed representation of the transaction to the given writer.
func (ud *Undelegate) PrettyPrint(_ context.Context, prefix string, w io.Writer) {
	fmt.Fprintf(w, "%sFrom: %s\n", prefix, ud.From)
	fmt.Fprintf(w, "%sShares: %s\n", prefix, ud.Shares)
}

// PrettyType returns a representation of the type that can be used for pretty printing.
func (ud *Undelegate) PrettyType() (interface{}, error) {
	return ud, nil
}

// BalanceQuery are the arguments for consensus.Balance method.
type BalanceQuery struct {
	Address types.Address `json:"address"`
}

// AccountBalance is the consensus balance in an account.
type AccountBalance struct {
	Balance types.Quantity `json:"balance"`
}

// AccountQuery are the arguments for consensus.Account method.
type AccountQuery struct {
	Address types.Address `json:"address"`
}

// DelegationQuery are the arguments for consensus.Delegation method.
type DelegationQuery struct {
	From types.Address `json:"from"`
	To   types.Address `json:"to"`
}

// DelegationsQuery are the arguments for consensus.Delegations method.
type DelegationsQuery struct {
	From types.Address `json:"from"`
}

// UndelegationsQuery are the arguments for consensus.Undelegations method.
type UndelegationsQuery struct {
	To types.Address `json:"to"`
}

// DelegationInfo is information about a delegation.
type DelegationInfo struct {
	Shares types.Quantity `json:"shares"`
}

// ExtendedDelegationInfo is extended information about a delegation.
type ExtendedDelegationInfo struct {
	To     types.Address  `json:"to"`
	Shares types.Quantity `json:"shares"`
}

// UndelegationInfo is information about an undelegation.
type UndelegationInfo struct {
	From   types.Address    `json:"from"`
	Epoch  beacon.EpochTime `json:"epoch"`
	Shares types.Quantity   `json:"shares"`
}

// GasCosts are the consensus accounts module gas costs.
type GasCosts struct {
	TxDeposit    uint64 `json:"tx_deposit"`
	TxWithdraw   uint64 `json:"tx_withdraw"`
	TxDelegate   uint64 `json:"tx_delegate"`
	TxUndelegate uint64 `json:"tx_undelegate"`
}

// Parameters are the parameters for the consensus accounts module.
type Parameters struct {
	GasCosts GasCosts `json:"gas_costs"`

	DisableDelegate   bool `json:"disable_delegate"`
	DisableUndelegate bool `json:"disable_undelegate"`
	DisableDeposit    bool `json:"disable_deposit"`
	DisableWithdraw   bool `json:"disable_withdraw"`
}

// ConsensusError contains error details from the consensus layer.
type ConsensusError struct {
	Module string `json:"module,omitempty"`
	Code   uint32 `json:"code,omitempty"`
}

// ModuleName is the consensus accounts module name.
const ModuleName = "consensus_accounts"

const (
	// DepositEventCode is the event code for the deposit event.
	DepositEventCode = 1
	// WithdrawEventCode is the event code for the withdraw event.
	WithdrawEventCode = 2
	// DelegateEventCode is the event code for the delegate event.
	DelegateEventCode = 3
	// UndelegateStartEventCode is the event code for the undelegate start event.
	UndelegateStartEventCode = 4
	// UndelegateDoneEventCode is the event code for the undelegate done event.
	UndelegateDoneEventCode = 5
)

// DepositEvent is a deposit event.
type DepositEvent struct {
	From   types.Address   `json:"from"`
	Nonce  uint64          `json:"nonce"`
	To     types.Address   `json:"to"`
	Amount types.BaseUnits `json:"amount"`
	Error  *ConsensusError `json:"error,omitempty"`
}

// IsSuccess checks whether the event indicates a successful operation.
func (de *DepositEvent) IsSuccess() bool {
	return de.Error == nil
}

// WithdrawEvent is a withdraw event.
type WithdrawEvent struct {
	From   types.Address   `json:"from"`
	Nonce  uint64          `json:"nonce"`
	To     types.Address   `json:"to"`
	Amount types.BaseUnits `json:"amount"`
	Error  *ConsensusError `json:"error,omitempty"`
}

// IsSuccess checks whether the event indicates a successful operation.
func (we *WithdrawEvent) IsSuccess() bool {
	return we.Error == nil
}

// DelegateEvent is a delegate event.
type DelegateEvent struct {
	From   types.Address   `json:"from"`
	Nonce  uint64          `json:"nonce"`
	To     types.Address   `json:"to"`
	Amount types.BaseUnits `json:"amount"`
	Error  *ConsensusError `json:"error,omitempty"`
}

// IsSuccess checks whether the event indicates a successful operation.
func (we *DelegateEvent) IsSuccess() bool {
	return we.Error == nil
}

// UndelegateStartEvent is an undelegate start event.
type UndelegateStartEvent struct {
	From          types.Address    `json:"from"`
	Nonce         uint64           `json:"nonce"`
	To            types.Address    `json:"to"`
	Shares        types.Quantity   `json:"shares"`
	DebondEndTime beacon.EpochTime `json:"debond_end_time"`
	Error         *ConsensusError  `json:"error,omitempty"`
}

// IsSuccess checks whether the event indicates a successful operation.
func (we *UndelegateStartEvent) IsSuccess() bool {
	return we.Error == nil
}

// UndelegateDoneEvent is an undelegate done event.
type UndelegateDoneEvent struct {
	From   types.Address   `json:"from"`
	To     types.Address   `json:"to"`
	Shares types.Quantity  `json:"shares"`
	Amount types.BaseUnits `json:"amount"`
}

// Event is a consensus account event.
type Event struct {
	Deposit         *DepositEvent
	Withdraw        *WithdrawEvent
	Delegate        *DelegateEvent
	UndelegateStart *UndelegateStartEvent
	UndelegateDone  *UndelegateDoneEvent
}
