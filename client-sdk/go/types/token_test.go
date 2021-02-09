package types

import "testing"

func TestToken(t *testing.T) {
	// TODO
	/*let cases = vec![
	      // Native denomination.
	      (0, Denomination::NATIVE, "824040"),
	      (1, Denomination::NATIVE, "82410140"),
	      (1000, Denomination::NATIVE, "824203e840"),
	      // Custom denomination.
	      (0, "test".into(), "82404474657374"),
	      (1, "test".into(), "8241014474657374"),
	      (1000, "test".into(), "824203e84474657374"),
	  ];

	  for tc in cases {
	      let token = BaseUnits::new(Quantity::from(tc.0), tc.1);
	      let enc = cbor::to_vec(&token);
	      assert_eq!(hex::encode(&enc), tc.2, "serialization should match");

	      let dec: BaseUnits = cbor::from_slice(&enc).expect("deserialization should succeed");
	      assert_eq!(dec, token, "serialization should round-trip");
	  }*/
}
