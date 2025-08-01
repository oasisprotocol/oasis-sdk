use honggfuzz::fuzz;

use rofl_proxy::http::sni;

fn main() {
    loop {
        fuzz!(|data: &[u8]| {
            let _ = sni::parse(data);
        });
    }
}
