package inspect

import (
	"context"
	"encoding/csv"
	"fmt"
	"os"
	"strconv"

	"github.com/olekukonko/tablewriter"
	"github.com/spf13/cobra"

	"github.com/oasisprotocol/oasis-core/go/common/cbor"
	"github.com/oasisprotocol/oasis-core/go/common/crypto/signature"
	"github.com/oasisprotocol/oasis-core/go/common/node"
	consensus "github.com/oasisprotocol/oasis-core/go/consensus/api"
	"github.com/oasisprotocol/oasis-core/go/consensus/api/transaction"
	roothash "github.com/oasisprotocol/oasis-core/go/roothash/api"
	"github.com/oasisprotocol/oasis-core/go/roothash/api/block"
	"github.com/oasisprotocol/oasis-core/go/roothash/api/commitment"
	scheduler "github.com/oasisprotocol/oasis-core/go/scheduler/api"

	"github.com/oasisprotocol/oasis-sdk/cli/cmd/common"
	cliConfig "github.com/oasisprotocol/oasis-sdk/cli/config"
	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/connection"
)

type runtimeStats struct {
	// Rounds.
	rounds uint64
	// Successful rounds.
	successfulRounds uint64
	// Failed rounds.
	failedRounds uint64
	// Rounds failed due to proposer timeouts.
	proposerTimeoutedRounds uint64
	// Epoch transition rounds.
	epochTransitionRounds uint64
	// Suspended rounds.
	suspendedRounds uint64

	// Discrepancies.
	discrepancyDetected        uint64
	discrepancyDetectedTimeout uint64

	// Per-entity stats.
	entities map[signature.PublicKey]*entityStats

	entitiesOutput [][]string
	entitiesHeader []string
}

type entityStats struct {
	// Rounds entity node was elected.
	roundsElected uint64
	// Rounds entity node was elected as primary executor worker.
	roundsPrimary uint64
	// Rounds entity node was elected as primary executor worker and workers were invoked.
	roundsPrimaryRequired uint64
	// Rounds entity node was elected as a backup executor worker.
	roundsBackup uint64
	// Rounds entity node was elected as a backup executor worker
	// and backup workers were invoked.
	roundsBackupRequired uint64
	// Rounds entity node was a proposer.
	roundsProposer uint64

	// How many times entity node proposed a timeout.
	proposedTimeout uint64

	// How many good blocks committed while being primary worker.
	committeedGoodBlocksPrimary uint64
	// How many bad blocs committed while being primary worker.
	committeedBadBlocksPrimary uint64
	// How many good blocks committed while being backup worker.
	committeedGoodBlocksBackup uint64
	// How many bad blocks committed while being backup worker.
	committeedBadBlocksBackup uint64

	// How many rounds missed committing a block while being a primary worker.
	missedPrimary uint64
	// How many rounds missed committing a block while being a backup worker (and discrepancy detection was invoked).
	missedBackup uint64
	// How many rounds proposer timeout was triggered while being the proposer.
	missedProposer uint64
}

func (s *runtimeStats) prepareEntitiesOutput() {
	s.entitiesOutput = make([][]string, 0)

	s.entitiesHeader = []string{
		"Entity ID",
		"Elected",
		"Primary",
		"Backup",
		"Proposer",
		"Primary invoked",
		"Primary Good commit",
		"Prim Bad commmit",
		"Bckp invoked",
		"Bckp Good commit",
		"Bckp Bad commit",
		"Primary missed",
		"Bckp missed",
		"Proposer missed",
		"Proposed timeout",
	}

	for entity, stats := range s.entities {
		var line []string
		line = append(line,
			entity.String(),
			strconv.FormatUint(stats.roundsElected, 10),
			strconv.FormatUint(stats.roundsPrimary, 10),
			strconv.FormatUint(stats.roundsBackup, 10),
			strconv.FormatUint(stats.roundsProposer, 10),
			strconv.FormatUint(stats.roundsPrimaryRequired, 10),
			strconv.FormatUint(stats.committeedGoodBlocksPrimary, 10),
			strconv.FormatUint(stats.committeedBadBlocksPrimary, 10),
			strconv.FormatUint(stats.roundsBackupRequired, 10),
			strconv.FormatUint(stats.committeedGoodBlocksBackup, 10),
			strconv.FormatUint(stats.committeedBadBlocksBackup, 10),
			strconv.FormatUint(stats.missedPrimary, 10),
			strconv.FormatUint(stats.missedBackup, 10),
			strconv.FormatUint(stats.missedProposer, 10),
			strconv.FormatUint(stats.proposedTimeout, 10),
		)
		s.entitiesOutput = append(s.entitiesOutput, line)
	}
}

func (s *runtimeStats) printStats() {
	fmt.Printf("Runtime rounds: %d\n", s.rounds)
	fmt.Printf("Successful rounds: %d\n", s.successfulRounds)
	fmt.Printf("Epoch transition rounds: %d\n", s.epochTransitionRounds)
	fmt.Printf("Proposer timeouted rounds: %d\n", s.proposerTimeoutedRounds)
	fmt.Printf("Failed rounds: %d\n", s.failedRounds)
	fmt.Printf("Discrepancies: %d\n", s.discrepancyDetected)
	fmt.Printf("Discrepancies (timeout): %d\n", s.discrepancyDetectedTimeout)
	fmt.Printf("Suspended: %d\n", s.suspendedRounds)

	fmt.Println("Entity stats")
	table := tablewriter.NewWriter(os.Stdout)
	table.SetBorders(tablewriter.Border{Left: true, Top: false, Right: true, Bottom: false})
	table.SetCenterSeparator("|")
	table.SetHeader(s.entitiesHeader)
	table.AppendBulk(s.entitiesOutput)
	table.Render()
}

var runtimeStatsCmd = &cobra.Command{
	Use:   "runtime-stats [<start-height> [<end-height>]]",
	Short: "Show runtime statistics",
	Args:  cobra.MaximumNArgs(2),
	Run: func(cmd *cobra.Command, args []string) {
		cfg := cliConfig.Global()
		npw := common.GetNPWSelection(cfg)
		runtimeID := npw.ParaTime.Namespace()

		// Parse command line arguments
		var startHeight, endHeight uint64
		if argLen := len(args); argLen > 0 {
			var err error

			// Start height is present for 1 and 2 args.
			startHeight, err = strconv.ParseUint(args[0], 10, 64)
			cobra.CheckErr(err)

			if argLen == 2 {
				endHeight, err = strconv.ParseUint(args[1], 10, 64)
				cobra.CheckErr(err)
			}
		}

		// Establish connection with the target network.
		ctx := context.Background()
		conn, err := connection.Connect(ctx, npw.Network)
		cobra.CheckErr(err)

		consensusConn := conn.Consensus()

		// Fixup the start/end heights if they were not specified (or are 0)
		if startHeight == 0 {
			var status *consensus.Status
			status, err = consensusConn.GetStatus(ctx)
			cobra.CheckErr(err)
			startHeight = uint64(status.LastRetainedHeight)
		}
		if endHeight == 0 {
			var blk *consensus.Block
			blk, err = consensusConn.GetBlock(ctx, consensus.HeightLatest)
			cobra.CheckErr(err)
			endHeight = uint64(blk.Height)
		}

		chainCtx, err := consensusConn.GetChainContext(ctx)
		cobra.CheckErr(err)
		signature.SetChainContext(chainCtx)

		fmt.Printf(
			"gathering statistics: runtime_id: %s, start_height: %d, end_height: %d\n",
			runtimeID,
			startHeight,
			endHeight,
		)

		// Do the actual work
		stats := &runtimeStats{
			entities: make(map[signature.PublicKey]*entityStats),
		}

		var (
			currentRound     uint64
			currentCommittee *scheduler.Committee
			currentScheduler *scheduler.CommitteeNode
			roundDiscrepancy bool
		)

		roothashConn := consensusConn.RootHash()
		registryConn := consensusConn.Registry()
		nodeToEntity := make(map[signature.PublicKey]signature.PublicKey)

		for height := int64(startHeight); height < int64(endHeight); height++ {
			if height%1000 == 0 {
				fmt.Printf("progressed: height: %d\n", height)
			}
			// Update node to entity map.
			var nodes []*node.Node
			nodes, err = registryConn.GetNodes(ctx, height)
			cobra.CheckErr(err)
			for _, node := range nodes {
				nodeToEntity[node.ID] = node.EntityID
			}

			rtRequest := &roothash.RuntimeRequest{
				RuntimeID: runtimeID,
				Height:    height,
			}

			// Query latest roothash block and events.
			var blk *block.Block
			blk, err = roothashConn.GetLatestBlock(ctx, rtRequest)
			switch err {
			case nil:
			case roothash.ErrInvalidRuntime:
				continue
			default:
				cobra.CheckErr(err)
			}
			var evs []*roothash.Event
			evs, err = roothashConn.GetEvents(ctx, height)
			cobra.CheckErr(err)

			var proposerTimeout bool
			if currentRound != blk.Header.Round && currentCommittee != nil {
				// If new round, check for proposer timeout.
				// Need to look at submitted transactions if round failure was caused by a proposer timeout.
				var rsp *consensus.TransactionsWithResults
				rsp, err = consensusConn.GetTransactionsWithResults(ctx, height)
				cobra.CheckErr(err)
				for i := 0; i < len(rsp.Transactions); i++ {
					// Ignore failed txs.
					if !rsp.Results[i].IsSuccess() {
						continue
					}
					var sigTx transaction.SignedTransaction
					err = cbor.Unmarshal(rsp.Transactions[i], &sigTx)
					cobra.CheckErr(err)
					var tx transaction.Transaction
					err = sigTx.Open(&tx)
					cobra.CheckErr(err)
					// Ignore non proposer timeout txs.
					if tx.Method != roothash.MethodExecutorProposerTimeout {
						continue
					}
					var xc roothash.ExecutorProposerTimeoutRequest
					err = cbor.Unmarshal(tx.Body, &xc)
					cobra.CheckErr(err)
					// Ignore txs of other runtimes.
					if xc.ID != runtimeID {
						continue
					}
					// Proposer timeout triggered the round failure, update stats.
					stats.entities[nodeToEntity[sigTx.Signature.PublicKey]].proposedTimeout++
					stats.entities[nodeToEntity[currentScheduler.PublicKey]].missedProposer++
					proposerTimeout = true
					break
				}
			}

			// Go over events before updating potential new round committee info.
			// Even if round transition happened at this height, all events emitted
			// at this height belong to the previous round.
			for _, ev := range evs {
				// Skip events for initial height where we don't have round info yet.
				if height == int64(startHeight) {
					break
				}
				// Skip events for other runtimes.
				if ev.RuntimeID != runtimeID {
					continue
				}
				switch {
				case ev.ExecutorCommitted != nil:
					// Nothing to do here. We use Finalized event Good/Bad Compute node
					// fields to process commitments.
				case ev.ExecutionDiscrepancyDetected != nil:
					if ev.ExecutionDiscrepancyDetected.Timeout {
						stats.discrepancyDetectedTimeout++
					} else {
						stats.discrepancyDetected++
					}
					roundDiscrepancy = true
				case ev.Finalized != nil:
					var rtResults *roothash.RoundResults
					rtResults, err = roothashConn.GetLastRoundResults(ctx, rtRequest)
					cobra.CheckErr(err)

					// Skip the empty finalized event that is triggered on initial round.
					if len(rtResults.GoodComputeEntities) == 0 && len(rtResults.BadComputeEntities) == 0 && currentCommittee == nil {
						continue
					}
					// Skip if epoch transition or suspended blocks.
					if blk.Header.HeaderType == block.EpochTransition || blk.Header.HeaderType == block.Suspended {
						continue
					}
					// Skip if proposer timeout.
					if proposerTimeout {
						continue
					}

					// Update stats.
				OUTER:
					for _, member := range currentCommittee.Members {
						entity := nodeToEntity[member.PublicKey]
						// Primary workers are always required.
						if member.Role == scheduler.RoleWorker {
							stats.entities[entity].roundsPrimaryRequired++
						}
						// In case of discrepancies backup workers were invoked as well.
						if roundDiscrepancy && member.Role == scheduler.RoleBackupWorker {
							stats.entities[entity].roundsBackupRequired++
						}

						// Go over good commitments.
						for _, v := range rtResults.GoodComputeEntities {
							if entity != v {
								continue
							}
							switch member.Role {
							case scheduler.RoleWorker:
								stats.entities[entity].committeedGoodBlocksPrimary++
								continue OUTER
							case scheduler.RoleBackupWorker:
								if roundDiscrepancy {
									stats.entities[entity].committeedGoodBlocksBackup++
									continue OUTER
								}
							case scheduler.RoleInvalid:
							}
						}

						// Go over bad commitments.
						for _, v := range rtResults.BadComputeEntities {
							if entity != v {
								continue
							}
							switch member.Role {
							case scheduler.RoleWorker:
								stats.entities[entity].committeedBadBlocksPrimary++
								continue OUTER
							case scheduler.RoleBackupWorker:
								if roundDiscrepancy {
									stats.entities[entity].committeedBadBlocksBackup++
									continue OUTER
								}
							case scheduler.RoleInvalid:
							}
						}

						// Neither good nor bad - missed commitment.
						if member.Role == scheduler.RoleWorker {
							stats.entities[entity].missedPrimary++
						}
						if roundDiscrepancy && member.Role == scheduler.RoleBackupWorker {
							stats.entities[entity].missedBackup++
						}
					}
				}
			}

			// New round.
			if currentRound != blk.Header.Round {
				currentRound = blk.Header.Round
				stats.rounds++

				switch blk.Header.HeaderType {
				case block.Normal:
					stats.successfulRounds++
				case block.EpochTransition:
					stats.epochTransitionRounds++
				case block.RoundFailed:
					if proposerTimeout {
						stats.proposerTimeoutedRounds++
					} else {
						stats.failedRounds++
					}
				case block.Suspended:
					stats.suspendedRounds++
					currentCommittee = nil
					currentScheduler = nil
					continue
				default:
					cobra.CheckErr(fmt.Errorf(
						"unexpected block header type: header_type: %v, height: %v",
						blk.Header.HeaderType,
						height,
					))
				}

				// Query runtime state and setup committee info for the round.
				var state *roothash.RuntimeState
				state, err = roothashConn.GetRuntimeState(ctx, rtRequest)
				cobra.CheckErr(err)
				if state.ExecutorPool == nil {
					// No committee - election failed(?)
					fmt.Printf("\nWarning: unexpected or missing committee for runtime: height: %d\n", height)
					currentCommittee = nil
					currentScheduler = nil
					continue
				}
				// Set committee info.
				currentCommittee = state.ExecutorPool.Committee
				currentScheduler, err = commitment.GetTransactionScheduler(currentCommittee, currentRound)
				cobra.CheckErr(err)
				roundDiscrepancy = false

				// Update election stats.
				seen := make(map[signature.PublicKey]bool)
				for _, member := range currentCommittee.Members {
					entity := nodeToEntity[member.PublicKey]
					if _, ok := stats.entities[entity]; !ok {
						stats.entities[entity] = &entityStats{}
					}

					// Multiple records for same node in case the node has
					// multiple roles. Only count it as elected once.
					if !seen[member.PublicKey] {
						stats.entities[entity].roundsElected++
					}
					seen[member.PublicKey] = true

					if member.Role == scheduler.RoleWorker {
						stats.entities[entity].roundsPrimary++
					}
					if member.Role == scheduler.RoleBackupWorker {
						stats.entities[entity].roundsBackup++
					}
					if member.PublicKey == currentScheduler.PublicKey {
						stats.entities[entity].roundsProposer++
					}
				}
			}
		}

		// Prepare and printout stats.
		stats.prepareEntitiesOutput()
		stats.printStats()

		// Also save entity stats in a csv.
		fout, err := os.Create(fmt.Sprintf("runtime-%s-%d-%d-stats.csv", runtimeID, startHeight, endHeight))
		cobra.CheckErr(err)
		defer fout.Close()

		w := csv.NewWriter(fout)
		err = w.Write(stats.entitiesHeader)
		cobra.CheckErr(err)
		err = w.WriteAll(stats.entitiesOutput)
		cobra.CheckErr(err)
	},
}
