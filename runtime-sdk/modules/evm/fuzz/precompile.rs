use honggfuzz::fuzz;

use oasis_runtime_sdk_evm::precompile::testing::call_contract;

fn main() {
    loop {
        fuzz!(|data: &[u8]| {
            // Simple encoding to make corpus generation easier: <a0> <a18> <a19> <input...>
            if data.len() < 3 {
                return;
            }

            let mut address = [0u8; 20];
            address[0] = data[0] % 2;
            address[18] = data[1] % 2;
            address[19] = data[2] % 11;
            let data = &data[3..];

            call_contract(address.into(), data, 1_000_000);
        });
    }
}
