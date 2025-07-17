import { bech32 } from "bech32";
import { task } from 'hardhat/config';

task("deploy", "Deploy the oracle contract")
  .addPositionalParam("roflAppID", "ROFL App ID")
  .setAction(async ({ roflAppID }, hre) => {
    const threshold = 1; // Number of app instances required to submit observations.

    // TODO: Move below to a ROFL helper library (@oasisprotocol/rofl).
    // const rawAppID = rofl.parseAppID(roflAppID);

    const {prefix, words} = bech32.decode(roflAppID);
    if (prefix !== "rofl") {
      throw new Error(`Malformed ROFL app identifier: ${roflAppID}`);
    }
    const rawAppID = new Uint8Array(bech32.fromWords(words));

    // Deploy a new instance of the oracle contract configuring the ROFL app that is
    // allowed to submit observations and the number of app instances required.
    const oracle = await hre.ethers.deployContract("Oracle", [rawAppID, threshold], {});
    await oracle.waitForDeployment();

    console.log(`Oracle for ROFL app ${roflAppID} deployed to ${oracle.target}`);
  });

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
