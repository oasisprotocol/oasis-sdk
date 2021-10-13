package cmd

import (
	"context"
	"fmt"
	"os"
	"os/signal"
	"strings"
	"time"

	"github.com/prometheus/client_golang/prometheus"
	"github.com/prometheus/client_golang/prometheus/push"
	"github.com/spf13/cobra"
	"github.com/spf13/viper"

	"github.com/oasisprotocol/oasis-core/go/common/logging"
	"github.com/oasisprotocol/oasis-core/go/oasis-node/cmd/common/grpc"
	cmdGrpc "github.com/oasisprotocol/oasis-core/go/oasis-node/cmd/common/grpc"

	"github.com/oasisprotocol/oasis-sdk/tests/benchmark/benchmarks/accounts"
	"github.com/oasisprotocol/oasis-sdk/tests/benchmark/benchmarks/api"
)

const (
	cfgBenchmarks            = "benchmarks"
	cfgBenchmarksConcurrency = "benchmarks.concurrency"
	cfgBenchmarksDuration    = "benchmarks.duration"
	cfgBenchmarksRate        = "benchmarks.rate"

	cfgLogLevel = "log.level"
	cfgLogFile  = "log.file"

	cfgPrometheusPushAddr          = "prometheus.push.addr"
	cfgPrometheusPushJobName       = "prometheus.push.job_name"
	cfgPrometheusPushInstanceLabel = "prometheus.push.instance_label"
)

var (
	flagBenchmarks            benchmarkValues
	flagRuntimeID             string
	flagBenchmarksConcurrency uint
	flagBenchmarksDuration    time.Duration
	flagBenchmarksRate        uint

	flagPrometheusPushAddr          string
	flagPrometheusPushJobName       string
	flagPrometheusPushInstanceLabel string
)

func benchmarkMain(cmd *cobra.Command, args []string) {
	logger := logging.GetLogger("benchmarks")

	flagBenchmarks.deduplicate()
	if len(flagBenchmarks.benchmarks) == 0 {
		logger.Error("insufficient benchmarks requested")
		os.Exit(1)
	}

	// Build the config.
	var cfg api.Config
	cfg.Logger = logger

	cfg.Duration, _ = cmd.Flags().GetDuration(cfgBenchmarksDuration)
	concurrency, _ := cmd.Flags().GetUint(cfgBenchmarksConcurrency)
	if concurrency == 0 {
		concurrency = 1
	}
	cfg.Concurrency = int(concurrency)

	cfg.Rate, _ = cmd.Flags().GetUint(cfgBenchmarksRate)

	if err := cfg.RuntimeID.UnmarshalHex(flagRuntimeID); err != nil {
		logger.Error("invalid runtime ID",
			"runtime_id", cfgRuntimeID,
			"err", err,
		)
		os.Exit(1)

	}
	conn, err := cmdGrpc.NewClient(cmd)
	if err != nil {
		logger.Error("failed to establish connection with node",
			"err", err,
		)
		os.Exit(1)
	}
	cfg.Conn = conn

	sigCh := make(chan os.Signal)
	signal.Notify(sigCh, os.Interrupt)
	ctx, cancelFn := context.WithCancel(context.Background())
	go func() {
		<-sigCh
		logger.Error("user requested interrupt")
		cancelFn()
	}()

	for _, benchmark := range flagBenchmarks.benchmarks {
		if err := cfg.RunBenchmark(ctx, benchmark); err != nil {
			if err == context.Canceled {
				break
			}

			logger.Error("failed to run benchmark",
				"err", err,
				"benchmark", benchmark.Name(),
			)
			os.Exit(1)
		}
	}

	if err := pushMetrics(cmd); err != nil {
		logger.Error("failed to push metrics",
			"err", err,
		)
	}
}

func pushMetrics(cmd *cobra.Command) error {
	addr, _ := cmd.Flags().GetString(cfgPrometheusPushAddr)
	if addr == "" {
		return nil
	}

	jobName, _ := cmd.Flags().GetString(cfgPrometheusPushJobName)
	if jobName == "" {
		return fmt.Errorf("metrics: %v required for metrics push mode", cfgPrometheusPushJobName)
	}
	instanceLabel, _ := cmd.Flags().GetString(cfgPrometheusPushInstanceLabel)
	if instanceLabel == "" {
		return fmt.Errorf("metrics: %v required for metrics push mode", cfgPrometheusPushInstanceLabel)
	}

	pusher := push.New(addr, jobName).Grouping("instance", instanceLabel).Gatherer(prometheus.DefaultGatherer)
	return pusher.Push()
}

type benchmarkValues struct {
	benchmarks []api.Benchmark
}

func (v *benchmarkValues) deduplicate() {
	var benchmarks []api.Benchmark
	seen := make(map[string]bool)
	for _, v := range v.benchmarks {
		name := v.Name()
		if !seen[name] {
			benchmarks = append(benchmarks, v)
			seen[name] = true
		}
	}

	v.benchmarks = benchmarks
}

func (v *benchmarkValues) String() string {
	var names []string
	for _, v := range v.benchmarks {
		names = append(names, v.Name())
	}

	return strings.Join(names, ",")
}

func (v *benchmarkValues) Set(sVec string) error {
	registeredBenchmarks := api.Benchmarks()

	for _, s := range strings.Split(sVec, ",") {
		bench, ok := registeredBenchmarks[s]
		if !ok {
			return fmt.Errorf("unknown benchmark: '%v'", s)
		}

		v.benchmarks = append(v.benchmarks, bench)
	}

	return nil
}

func (v *benchmarkValues) Type() string {
	registeredBenchmarks := api.Benchmarks()

	var benchmarks []string
	for name := range registeredBenchmarks {
		benchmarks = append(benchmarks, name)
	}

	return strings.Join(benchmarks, ",")
}

func benchmarkInit(cmd *cobra.Command) {
	cmd.Flags().VarP(&flagBenchmarks, cfgBenchmarks, "b", "Benchmarks")
	cmd.Flags().StringVar(&flagRuntimeID, cfgRuntimeID, "", "Benchmarked runtime ID (HEX)")
	cmd.Flags().UintVar(&flagBenchmarksConcurrency, cfgBenchmarksConcurrency, 1, "Benchmark concurrency")
	cmd.Flags().DurationVar(&flagBenchmarksDuration, cfgBenchmarksDuration, 30*time.Second, "Benchmark duration")
	cmd.Flags().UintVar(&flagBenchmarksRate, cfgBenchmarksRate, 1, "Benchmark maximum per second rate per concurrent connection")
	cmd.Flags().StringVar(&flagPrometheusPushAddr, cfgPrometheusPushAddr, "", "Prometheus push gateway address")
	cmd.Flags().StringVar(&flagPrometheusPushJobName, cfgPrometheusPushJobName, "", "Prometheus push `job` name")
	cmd.Flags().StringVar(&flagPrometheusPushInstanceLabel, cfgPrometheusPushInstanceLabel, "", "Prometheus push `instance` label")

	for _, v := range []string{
		cfgBenchmarks,
		cfgRuntimeID,
		cfgBenchmarksConcurrency,
		cfgBenchmarksDuration,
		cfgBenchmarksRate,
		cfgLogLevel,
		cfgLogFile,
		cfgPrometheusPushAddr,
		cfgPrometheusPushJobName,
		cfgPrometheusPushInstanceLabel,
	} {
		viper.BindPFlag(v, cmd.Flags().Lookup(v)) // nolint: errcheck
	}

	cmd.Flags().AddFlagSet(grpc.ClientFlags)

	for _, fn := range []api.SuiteInitFn{
		accounts.Init,
	} {
		fn(cmd)
	}
}
