// Package api implements the common interface exposed by all benchmarks.
package api

import (
	"context"
	"sync"
	"sync/atomic"
	"time"

	"github.com/prometheus/client_golang/prometheus"
	"github.com/spf13/cobra"
	"google.golang.org/grpc"

	"github.com/oasisprotocol/oasis-core/go/common"
	"github.com/oasisprotocol/oasis-core/go/common/logging"

	"github.com/oasisprotocol/oasis-sdk/client-sdk/go/client"
)

var benchmarkMap = make(map[string]Benchmark)

// Config is a benchmark run configuration.
type Config struct {
	Logger    *logging.Logger
	Conn      *grpc.ClientConn
	RuntimeID common.Namespace

	Concurrency     int
	Duration        time.Duration
	Rate            uint
	LogVerboseDebug bool
}

// RunBenchmark runs the benchmark with the provided configuration.
func (cfg *Config) RunBenchmark(ctx context.Context, benchmark Benchmark) error {
	logger := cfg.Logger.With("benchmark", benchmark.Name(), "runtime_id", cfg.RuntimeID)

	logger.Info("starting benchmark")

	rtc := client.New(cfg.Conn, cfg.RuntimeID)

	states := make([]*State, 0, cfg.Concurrency)
	defer func() {
		if bulkCleanupable, ok := benchmark.(BulkCleanupable); ok {
			bulkCleanupable.BulkCleanup(ctx, states)
		}

		for _, state := range states {
			if cleanupable, ok := benchmark.(Cleanupable); ok {
				cleanupable.Cleanup(state)
			}
		}
	}()

	// Prepare each benchmark go routine's state.
	for i := 0; i < cfg.Concurrency; i++ {
		var err error

		state := &State{
			Id:     uint64(i),
			Config: cfg,
			Logger: logger.With("goroutine", i),
			Client: rtc,
		}

		if prepareable, ok := benchmark.(Prepareable); ok {
			if err = prepareable.Prepare(ctx, state); err != nil {
				return err
			}
		}
		states = append(states, state)
	}
	logger.Info("preparation done")

	if bulkPreparable, ok := benchmark.(BulkPreparable); ok {
		if err := bulkPreparable.BulkPrepare(ctx, states); err != nil {
			return err
		}
		logger.Info("bulk-preparation done")
	}

	// Spawn each benchmark go routine.
	errCh := make(chan error, cfg.Concurrency)
	stopCh := make(chan struct{})
	counter := new(atomicCounter)
	var wg sync.WaitGroup

	var didHalt bool
	doHalt := func() {
		if !didHalt {
			close(stopCh)
			wg.Wait()
			didHalt = true
		}
	}
	defer doHalt()

	wg.Add(cfg.Concurrency)
	timeBefore := time.Now()

	var interval int64
	if cfg.Rate != 0 {
		interval = time.Second.Nanoseconds() / int64(cfg.Rate)
	}

	for i := 0; i < cfg.Concurrency; i++ {
		go func(state *State) {
			defer wg.Done()

			began, count := time.Now(), int64(0)
			for {
				select {
				case <-ctx.Done():
					logger.Debug("canceled")
					return
				case <-stopCh:
					logger.Debug("finished")
					return
				default:
				}

				iters, err := benchmark.Scenario(ctx, state)
				if err != nil {
					// The cancelation can also interrupt a scenario in
					// progress.
					if err == context.Canceled {
						logger.Debug("canceled")
						return
					}

					logger.Error("iteration failed",
						"err", err,
					)
					errCh <- err
					return
				}
				counter.Add(iters)

				if interval != 0 {
					// Rate limit
					now, next := time.Now(), began.Add(time.Duration(count*interval))
					time.Sleep(next.Sub(now))
					count++
				}

			}
		}(states[i])
	}

	duration := cfg.Duration
	timeStart := time.Now()
	countStart := counter.Get()
	logger.Info("threads started")

	doSleep := func(sleepDuration time.Duration, descr string) (time.Time, uint64, error) {
		logger.Info("begin " + descr)
		select {
		case <-ctx.Done():
			logger.Info("canceled during " + descr)
			return time.Time{}, 0, context.Canceled
		case err := <-errCh:
			return time.Time{}, 0, err
		case <-time.After(sleepDuration):
		}

		return time.Now(), counter.Get(), nil
	}

	// First 10% of time will be discarded.
	timeMidBefore, countMidBefore, err := doSleep(duration/10, "first 10%")
	if err != nil {
		return err
	}

	// Middle 80% of time will be counted.
	timeMidAfter, countMidAfter, err := doSleep(duration/10*8, "middle 80%")
	if err != nil {
		return err
	}

	// Last 10% of time will be discarded.
	timeEnd, countEnd, err := doSleep(duration/10, "last 10%")
	if err != nil {
		return err
	}

	// Signal end of run and wait for everything to finish.
	doHalt()
	timeAfter := time.Now()
	countAfter := counter.Get()
	logger.Info("threads joined")

	// Derive the actually useful results.
	midCount := countMidAfter - countMidBefore
	midDur := timeMidAfter.Sub(timeMidBefore)
	midDurMs := uint64(midDur / time.Millisecond)
	throughputInv := float64(midDurMs) / float64(midCount)
	throughput := float64(midCount) / midDur.Seconds()

	logger.Info("middle 80%",
		"calls", midCount,
		"duration", midDur,
		"calls_per_sec", throughput,
	)
	setAndRegisterGauge(benchmark.Name()+"_mid_count", float64(midCount))
	setAndRegisterGauge(benchmark.Name()+"_mid_dur_ms", float64(midDurMs))
	setAndRegisterGauge(benchmark.Name()+"_throughput_inv", throughputInv)
	setAndRegisterGauge(benchmark.Name()+"_throughput", throughput)

	// Log the optional (informative) extra results.
	totalCount := countEnd - countStart
	totalDur := timeEnd.Sub(timeStart)
	logger.Info("overall",
		"calls", totalCount,
		"duration", totalDur,
		"calls_per_sec", float64(totalCount)/totalDur.Seconds(),
	)

	beforeCount := countStart
	beforeDur := timeStart.Sub(timeBefore)
	logger.Info("ramp-up",
		"calls", beforeCount,
		"duration", beforeDur,
		"calls_per_sec", float64(beforeCount)/beforeDur.Seconds(),
	)

	afterCount := countAfter - countEnd
	afterDur := timeAfter.Sub(timeEnd)
	logger.Info("ramp-down",
		"calls", afterCount,
		"duration", afterDur,
		"calls_per_sec", float64(afterCount)/afterDur.Seconds(),
	)

	return nil
}

func setAndRegisterGauge(name string, value float64) {
	const prefix = "oasis_sdk_benchmark_"

	g := prometheus.NewGauge(
		prometheus.GaugeOpts{
			Name: prefix + name,
		},
	)
	prometheus.MustRegister(g)
	g.Set(value)
}

// Benchmark is the interface exposed by each benchmark.
type Benchmark interface {
	Name() string
	Scenario(context.Context, *State) (uint64, error)
}

// Prepareable is the interface exposed by benchmarks requiring a
// pre-flight prepare step.
type Prepareable interface {
	Prepare(context.Context, *State) error
}

// BulkPreparable is the interface exposed by benchmarks that require
// a bulk pre-flight prepare step.
//
// If a benchmark also is a Preparable, the BulkPrepare operation will
// be invoked after every Prepeare operation has been completed.
type BulkPreparable interface {
	BulkPrepare(context.Context, []*State) error
}

// BulkCleanupable is the interface exposed by benchmarks that require
// a bulk cleanup step.
//
// If a benchmark is also a Cleanupable, the BulkCleanup operation will
// be invoked before any Prepare operations are dispatched.
type BulkCleanupable interface {
	BulkCleanup(context.Context, []*State)
}

// Cleanupable is the interface exposed by benchmarks require a
// post-flight cleanup step.
type Cleanupable interface {
	Cleanup(*State)
}

// State is the per-goroutine benchmark state.
type State struct {
	Id     uint64
	Config *Config
	Logger *logging.Logger

	Client client.RuntimeClient

	State interface{}
}

// RegisterBenchmark registers a new benchmark.
func RegisterBenchmark(bench Benchmark) {
	name := bench.Name()
	if _, ok := benchmarkMap[name]; ok {
		panic("benchmark already registered: " + name)
	}
	benchmarkMap[name] = bench
}

// Benchmarks returns a map of all registered benchmarks.
func Benchmarks() map[string]Benchmark {
	return benchmarkMap
}

type atomicCounter struct {
	value uint64
}

func (c *atomicCounter) Get() uint64 {
	return atomic.LoadUint64(&c.value)
}

func (c *atomicCounter) Add(incr uint64) {
	atomic.AddUint64(&c.value, incr)
}

// SuiteInitFn is the initializer exposed by each benchmark suite package.
type SuiteInitFn func(*cobra.Command)
