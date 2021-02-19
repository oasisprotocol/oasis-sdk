# Notes for SDK development

## Method descriptors and wrappers

To get a rough set of method descriptors from a Go `ServiceDesc`, use a regular expression replacer
on the method entries:

```regexp
/\s*{\s*MethodName:\s*method(\w+)\.ShortName\(\),[^}]+},/g
```

For the method descriptor, replace with:

```js
"const methodDescriptor???$1 = createMethodDescriptorUnary<void, void>('???', '$1');\n"
```

For the wrapper, replace with:

```js
"???$1(arg: void) { return this.callUnary(methodDescriptor???$1, arg); }\n"
```

For stream entries:

```regexp
/\s*{\s*StreamName:\s*method(\w+)\.ShortName\(\),[^}]+},/g
```

For the method descriptor, replace with:

```js
"const methodDescriptor???$1 = createMethodDescriptorServerStreaming<void, void>('???', '$1');\n"
```

For the wrapper, replace with:

```js
"???$1(arg: void) { return this.callServerStreaming(methodDescriptor???$1, arg); }\n"
```

For example,

```go
			{
				MethodName: methodSubmitTx.ShortName(),
				Handler:    handlerSubmitTx,
			},
			{
				StreamName:    methodWatchBlocks.ShortName(),
				Handler:       handlerWatchBlocks,
				ServerStreams: true,
			},
```

becomes

```ts
const methodDescriptor???StateToGenesis = createMethodDescriptorUnary<void, void>('???', 'StateToGenesis');
const methodDescriptor???WatchBlocks = createMethodDescriptorServerStreaming<void, void>('???', 'WatchBlocks');
...
???SubmitTx(arg: void) { return this.callUnary(methodDescriptor???SubmitTx, arg); }
???WatchBlocks(arg: void) { return this.callServerStreaming(methodDescriptor???WatchBlocks, arg); }
```

Fill in the service name in the `???`s and the request and response types.
Fill in the wrapper parameter name as desired, usually based on the Go gRPC client wrapper.
Change void calls to pass a literal `undefined` request.

```ts
const methodDescriptorConsensusStateToGenesis = createMethodDescriptorUnary<types.longnum, types.GenesisDocument>('Consensus', 'StateToGenesis');
const methodDescriptorConsensusWatchBlocks = createMethodDescriptorServerStreaming<void, types.ConsensusBlock>('Consensus', 'WatchBlocks');
...
consensusSubmitTx(tx: types.SignatureSigned) { return this.callUnary(methodDescriptorConsensusSubmitTx, tx); }
consensusWatchBlocks() { return this.callServerStreaming(methodDescriptorConsensusWatchBlocks, undefined); }
```

## Error codes

To convert a block of error registrations, use this [CyberChef](https://gchq.github.io/CyberChef/)
recipe

```js
Regular_expression('User defined','Err\\w+\\s*=\\s*errors\\.New\\(\\w+, \\d+, ".*"\\)',true,true,false,false,false,false,'List matches')
Find_/_Replace({'option':'Regex','string':'Err(\\w+)\\s*=\\s*errors\\.New\\(\\w+, (\\d+), ".*"\\)'},'$1 = $2;',true,false,true,false)
Find_/_Replace({'option':'Regex','string':'[A-Z]'},'_$&',true,false,true,false)
To_Upper_case('All')
Find_/_Replace({'option':'Regex','string':'^_'},'export const CODE_',true,false,true,false)
```

For example,

```go
	// ErrNoCommittedBlocks is the error returned when there are no committed
	// blocks and as such no state can be queried.
	ErrNoCommittedBlocks = errors.New(moduleName, 1, "consensus: no committed blocks")
```

becomes 

```ts
export const CODE_NO_COMMITTED_BLOCKS = 1;
```

Fix up any uppercase parts that mess up the heuristic, e.g. `ErrTEEHardwareMismatch` ➡️
`CODE_T_E_E_HARDWARE_MISMATCH` ➡️ `CODE_TEE_HARDWARE_MISMATCH`.
