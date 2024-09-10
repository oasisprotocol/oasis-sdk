import { HardhatUserConfig } from "hardhat/config";
import '@oasisprotocol/sapphire-hardhat';
import "@nomicfoundation/hardhat-toolbox";

import "./tasks/query";

const config: HardhatUserConfig = {
  solidity: "0.8.19",
  networks: {
    'sapphire-testnet': {
      url: "https://testnet.sapphire.oasis.io",
      accounts: process.env.PRIVATE_KEY
        ? [process.env.PRIVATE_KEY]
        : [],
      chainId: 0x5aff,
    },
  },
};

export default config;
