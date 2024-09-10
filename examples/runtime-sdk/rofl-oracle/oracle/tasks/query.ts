import { bech32 } from "bech32";

task("oracle-query", "Queries the oracle contract")
  .addPositionalParam("contractAddress", "The deployed contract address")
  .setAction(async ({ contractAddress }, { ethers }) => {
    const oracle = await ethers.getContractAt("Oracle", contractAddress);

    console.log(`Using oracle contract deployed at ${oracle.target}`);

    const rawRoflAppID = await oracle.roflAppID();
    // TODO: Move below to a ROFL helper library (@oasisprotocol/rofl).
    const roflAppID = bech32.encode("rofl", bech32.toWords(ethers.getBytes(rawRoflAppID)));
    const threshold = await oracle.threshold();
    console.log(`ROFL app:  ${roflAppID}`);
    console.log(`Threshold: ${threshold}`);

    try {
      const [value, blockNum] = await oracle.getLastObservation();
      console.log(`Last observation: ${value}`);
      console.log(`Last update at:   ${blockNum}`);
    } catch {
      console.log(`No last observation available.`);
    }
  });
