# Simple ROFL Oracle Contract

For the ROFL Oracle smart contract deployment you will need the following
information:

- the ROFL app ID (rofl1...)
- the deployer's private key
- the network you're deploying to (sapphire-testnet, sapphire-localnet,
  sapphire)

First install dependencies and compile the smart contract:

```shell
npm install
npx hardhat compile
```

Then, prepare your hex-encoded private key for paying the deployment gas fee
and store it as an environment variable:

```shell
export PRIVATE_KEY=0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80
```

Finally, deploy the contract, provide the ROFL App ID and the network:

```shell
npx hardhat deploy rofl1qqn9xndja7e2pnxhttktmecvwzz0yqwxsquqyxdf --network sapphire-localnet
```

Once your Oracle ROFL is running, it will submit the observations to the smart
contract deployed above. You can fetch the data stored on-chain by running:

```shell
npx hardhat oracle-query 0x5FbDB2315678afecb367f032d93F642f64180aa3 --network sapphire-localnet
```

For more information check out the [ROFL tutorial].

[ROFL tutorial]: https://github.com/oasisprotocol/oasis-sdk/blob/main/docs/rofl/app.md

