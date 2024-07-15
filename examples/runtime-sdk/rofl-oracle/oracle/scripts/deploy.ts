import { ethers } from "hardhat";
import { bech32 } from "bech32";

async function main() {
  const roflAppID = process.env.ROFL_APP_ID;
  const threshold = 1; // Number of app instances required to submit observations.

  // TODO: Move below to a ROFL helper library (@oasisprotocol/rofl).
  // const rawAppID = rofl.parseAppID(roflAppID);
  if (!roflAppID) {
    throw new Error("ROFL app identifier (ROFL_APP_ID) not specified");
  }
  const {prefix, words} = bech32.decode(roflAppID);
  if (prefix !== "rofl") {
    throw new Error(`Malformed ROFL app identifier: ${roflAppID}`);
  }
  const rawAppID = new Uint8Array(bech32.fromWords(words));

  // Deploy a new instance of the oracle contract configuring the ROFL app that is
  // allowed to submit observations and the number of app instances required.
  const oracle = await ethers.deployContract("Oracle", [rawAppID, threshold], {});
  await oracle.waitForDeployment();

  console.log(`Oracle for ROFL app ${roflAppID} deployed to ${oracle.target}`);
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
