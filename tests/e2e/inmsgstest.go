package main

import (
	"context"
	"fmt"
	"time"

	"google.golang.org/grpc"

	"github.com/oasisprotocol/oasis-core/go/common/cbor"
	"github.com/oasisprotocol/oasis-core/go/common/logging"
	"github.com/oasisprotocol/oasis-core/go/common/quantity"
	consensus "github.com/oasisprotocol/oasis-core/go/consensus/api"
	"github.com/oasisprotocol/oasis-core/go/consensus/api/transaction"
	roothash "github.com/oasisprotocol/oasis-core/go/roothash/api"
	staking "github.com/oasisprotocol/oasis-core/go/staking/api"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/client"
	consensusAccounts "github.com/oasisprotocol/oasis-sdk/client-sdk/go/modules/consensusaccounts"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/testing"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/types"
)

func IncomingMessagesTest(sc *RuntimeScenario, log *logging.Logger, conn *grpc.ClientConn, rtc client.RuntimeClient) error {
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Minute)
	defer cancel()

	cons := consensus.NewConsensusClient(conn)
	stakingClient := cons.Staking()
	ch, sub, err := stakingClient.WatchEvents(ctx)
	defer sub.Close()
	if err != nil {
		return err
	}

	consDenomination := types.Denomination("TEST")

	consAccounts := consensusAccounts.NewV1(rtc)

	acCh, err := rtc.WatchEvents(ctx, []client.EventDecoder{consAccounts}, false)
	if err != nil {
		return err
	}

	runtimeAddr := staking.NewRuntimeAddress(runtimeID)

	// Message with no embedded transaction.
	signedTx, err := transaction.Sign(testing.Alice.ConsensusSigner, roothash.NewSubmitMsgTx(0, nil, &roothash.SubmitMsg{
		ID:     runtimeID,
		Tag:    0,
		Fee:    *quantity.NewFromUint64(1),
		Tokens: *quantity.NewFromUint64(10),
		Data:   cbor.Marshal(types.NoopIncomingMessageData()),
	}))
	if err != nil {
		return err
	}
	if err = cons.SubmitTx(ctx, signedTx); err != nil {
		return err
	}
	aliceAccount, err := cons.Staking().Account(ctx, &staking.OwnerQuery{
		Height: consensus.HeightLatest,
		Owner:  testing.Alice.Address.ConsensusAddress(),
	})
	// todo: figure out what consensus balance should be. 1 million minus something from previous tests
	expectedBalance := quantity.NewFromUint64(89)
	if aliceAccount.General.Balance.Cmp(expectedBalance) != 0 {
		return fmt.Errorf("after message 1: alice consensus balance expected %v actual %v", expectedBalance, aliceAccount.General.Balance)
	}
	// todo: need event to watch for mint...

	// todo: test other cases
	// - embedded transfer, different sender: should execute
	// - malformed data field: funds should work
	// - invalid transaction: funds should work
	// - failed transaction: funds should work
	// - too much has: funds should work

	return nil
}
