curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
rustup show
rustup target add x86_64-unknown-linux-musl
sudo apt update #missing from docs
sudo apt -y install musl-tools gcc-multilib clang
sudo apt -y install pkg-config protobuf-compiler cmake #missing from docs
cargo install fortanix-sgx-tools
cargo install sgxs-tools

wget https://github.com/oasisprotocol/cli/releases/download/v0.10.2/oasis_cli_0.10.2_linux_amd64.tar.gz
tar -zxvf oasis_cli_0.10.2_linux_amd64.tar.gz
sudo mv ./oasis_cli_0.10.2_linux_amd64/oasis /usr/local/bin/oasis

oasis rofl build sgx --mode unsafe
