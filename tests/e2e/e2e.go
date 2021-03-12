// End-to-end test harness using oasis-test-runner.
package main

import (
	"github.com/oasisprotocol/oasis-core/go/oasis-node/cmd/common"
	"github.com/oasisprotocol/oasis-core/go/oasis-test-runner/cmd"
)

func main() {
	if err := RegisterScenarios(); err != nil {
		common.EarlyLogAndExit(err)
	}

	cmd.Execute()
}
