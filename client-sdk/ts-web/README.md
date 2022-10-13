# Outer workspace for packages

This directory is an
[npm workspace](https://docs.npmjs.com/cli/v8/using-npm/workspaces) that helps
us link the contained packages into each other's dependencies.

## Building the packages

In this directory, run:

```sh
npm i --foreground-scripts
```

The `--foreground-scripts` option instructs npm to compile (TypeScript)
depended-on packages before compiling packages that depend on them.
[Tracked](https://github.com/npm/cli/issues/4100).
