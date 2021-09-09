//! Testing helpers.

pub mod addresses {
    pub mod alice {
        use crate::address::Address;

        pub fn address() -> Address {
            Address::from_bech32("oasis1qrec770vrek0a9a5lcrv0zvt22504k68svq7kzve").unwrap()
        }
    }

    pub mod bob {
        use crate::address::Address;

        pub fn address() -> Address {
            Address::from_bech32("oasis1qrydpazemvuwtnp3efm7vmfvg3tde044qg6cxwzx").unwrap()
        }
    }

    pub mod charlie {
        use crate::address::Address;

        pub fn address() -> Address {
            Address::from_bech32("oasis1qr5kfjm8lx6mctjmwcx9225q5k3nxacqwqnjahkw").unwrap()
        }
    }

    pub mod dave {
        use crate::address::Address;

        pub fn address() -> Address {
            Address::from_bech32("oasis1qpufkctqruam5umugwn5jvxtrvvwl075rqrmxqmm").unwrap()
        }
    }
}
