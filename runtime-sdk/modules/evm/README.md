# EVM Module

This directory contains the EVM module for the Oasis SDK. It allows execution
of EVM-compatible smart contracts.

## Known Divergence from Ethereum

* `SELFDISTRUCT` op code is unsupported. Invoking `SELFDESTRUCT` will result in
  a transaction being reverted. Solving this would require either inefficient
  iteration over all storage keys, a special storage operation for removing
  prefixes or some form of generational storage.

* `COINBASE` op code always returns an all-zero address.

* `DIFFICULTY` op code always returns zero.
