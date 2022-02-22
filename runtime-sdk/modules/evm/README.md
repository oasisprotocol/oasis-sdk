# EVM Module

This directory contains the EVM module for the Oasis SDK. It allows execution
of EVM-compatible smart contracts.

## Known Divergence from Ethereum

* `SELFDISTRUCT` is unsupported an invoking the `SELFDESTRUCT` opcode will
  result in a transaction being reverted. Solving this would require either
  inefficient iteration over all storage keys, a special storage operation for
  removing prefixes or some form of generational storage.
