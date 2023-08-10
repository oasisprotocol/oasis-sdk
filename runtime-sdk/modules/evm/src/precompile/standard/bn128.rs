use bn::{pairing_batch, AffineG1, AffineG2, Fq, Fq2, Fr, Group, Gt, G1, G2};
use evm::{
    executor::stack::{PrecompileFailure, PrecompileHandle, PrecompileOutput},
    ExitError, ExitSucceed,
};

use crate::precompile::{read_input, PrecompileResult};

/// The gas cost for point addition on the alt-bn128 elliptic curve.
///
/// See https://eips.ethereum.org/EIPS/eip-1108#specification.
const BN128_ADD_GAS_COST: u64 = 150;

/// The gas cost for point multiplication on the alt-bn128 elliptic curve.
///
/// See https://eips.ethereum.org/EIPS/eip-1108#specification.
const BN128_MUL_GAS_COST: u64 = 6000;

/// The base gas cost for pairing on the alt-bn128 elliptic curve.
///
/// See https://eips.ethereum.org/EIPS/eip-1108#specification.
const BN128_PAIRING_BASE_GAS_COST: u64 = 45_000;

/// The check gas cost per pairing on the alt-bn128 elliptic curve.
///
/// See https://eips.ethereum.org/EIPS/eip-1108#specification.
const BN128_PAIRING_CHECK_GAS_COST: u64 = 34_000;

/// Point addition on the alt-bn128 elliptic curve.
pub fn call_bn128_add(handle: &mut impl PrecompileHandle) -> PrecompileResult {
    handle.record_cost(BN128_ADD_GAS_COST)?;

    let input = handle.input();
    let p1 = read_curve_point_g1(input, 0)?;
    let p2 = read_curve_point_g1(input, 64)?;
    let sum = p1 + p2;
    let result = encode_curve_point_g1(sum);

    Ok(PrecompileOutput {
        exit_status: ExitSucceed::Returned,
        output: result.to_vec(),
    })
}

/// Point multiplication on the alt-bn128 elliptic curve.
pub fn call_bn128_mul(handle: &mut impl PrecompileHandle) -> PrecompileResult {
    handle.record_cost(BN128_MUL_GAS_COST)?;

    let input = handle.input();
    let p = read_curve_point_g1(input, 0)?;
    let s = read_field_element_fr(input, 64)?;
    let mul = p * s;
    let result = encode_curve_point_g1(mul);

    Ok(PrecompileOutput {
        exit_status: ExitSucceed::Returned,
        output: result.to_vec(),
    })
}

/// Pairing check on the alt-bn128 elliptic curve.
pub fn call_bn128_pairing(handle: &mut impl PrecompileHandle) -> PrecompileResult {
    let length = handle.input().len();
    if length % 192 > 0 {
        return Err(PrecompileFailure::Error {
            exit_status: ExitError::Other("bad elliptic curve pairing size".into()),
        });
    }

    let num_pairings = length / 192;
    handle.record_cost(
        BN128_PAIRING_BASE_GAS_COST + BN128_PAIRING_CHECK_GAS_COST * num_pairings as u64,
    )?;

    let input = handle.input();
    let mut pairs = Vec::with_capacity(num_pairings);

    for i in 0..num_pairings {
        let a = read_curve_point_g1(input, i * 192)?;
        let b = read_twist_point_g2(input, i * 192 + 64)?;
        pairs.push((a, b));
    }
    let mul = pairing_batch(&pairs);

    let mut result = [0u8; 32];
    if mul == Gt::one() {
        result[31] = 1;
    }

    Ok(PrecompileOutput {
        exit_status: ExitSucceed::Returned,
        output: result.to_vec(),
    })
}

/// Decodes elliptic curve point G1.
fn read_curve_point_g1(source: &[u8], offset: usize) -> Result<G1, PrecompileFailure> {
    let x = read_field_element_fq(source, offset)?;
    let y = read_field_element_fq(source, offset + 32)?;

    if x.is_zero() && y.is_zero() {
        return Ok(G1::zero());
    }

    Ok(AffineG1::new(x, y)
        .map_err(|_| PrecompileFailure::Error {
            exit_status: ExitError::Other("invalid point G1".into()),
        })?
        .into())
}

/// Decodes elliptic curve point G2.
fn read_twist_point_g2(source: &[u8], offset: usize) -> Result<G2, PrecompileFailure> {
    let ay = read_field_element_fq(source, offset)?;
    let ax = read_field_element_fq(source, offset + 32)?;
    let by = read_field_element_fq(source, offset + 64)?;
    let bx = read_field_element_fq(source, offset + 96)?;

    let x = Fq2::new(ax, ay);
    let y = Fq2::new(bx, by);

    if x.is_zero() && y.is_zero() {
        return Ok(G2::zero());
    }

    Ok(AffineG2::new(x, y)
        .map_err(|_| PrecompileFailure::Error {
            exit_status: ExitError::Other("invalid point G2".into()),
        })?
        .into())
}

/// Encodes elliptic curve point G1.
fn encode_curve_point_g1(p: G1) -> [u8; 64] {
    let mut result = [0u8; 64];
    match AffineG1::from_jacobian(p) {
        None => (), // Point at infinity.
        Some(p) => {
            p.x()
                .to_big_endian(&mut result[0..32])
                .expect("slice has 32-byte length");
            p.y()
                .to_big_endian(&mut result[32..64])
                .expect("slice has 32-byte length");
        }
    }
    result
}

/// Decodes field element Fq.
fn read_field_element_fq(source: &[u8], offset: usize) -> Result<Fq, PrecompileFailure> {
    let mut a = [0u8; 32];
    read_input(source, &mut a, offset);

    Fq::from_slice(&a).map_err(|_| PrecompileFailure::Error {
        exit_status: ExitError::Other("invalid field element Fq".into()),
    })
}

/// Decodes field element Fr.
fn read_field_element_fr(source: &[u8], offset: usize) -> Result<Fr, PrecompileFailure> {
    let mut a = [0u8; 32];
    read_input(source, &mut a, offset);

    Fr::from_slice(&a).map_err(|_| PrecompileFailure::Error {
        exit_status: ExitError::Other("invalid field element Fr".into()),
    })
}

#[cfg(test)]
mod test {
    use crate::precompile::testing::*;

    use super::*;

    #[test]
    fn test_bn128_add() {
        let address = H160([
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x06,
        ]);

        for case in read_test_cases("bn256Add").iter() {
            let ret = call_contract(
                address,
                &hex::decode(case.input.as_str()).unwrap(),
                case.gas,
            )
            .unwrap();
            assert_eq!(hex::encode(ret.unwrap().output), case.expected);
        }

        for case in read_test_cases("common_bnadd").iter() {
            let ret = call_contract(
                address,
                &hex::decode(case.input.as_str()).unwrap(),
                BN128_ADD_GAS_COST,
            )
            .unwrap();
            assert_eq!(hex::encode(ret.unwrap().output), case.expected);
        }
    }

    #[test]
    fn test_bn128_mul() {
        let address = H160([
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x07,
        ]);

        for case in read_test_cases("bn256ScalarMul").iter() {
            let ret = call_contract(
                address,
                &hex::decode(case.input.as_str()).unwrap(),
                case.gas,
            )
            .unwrap();
            assert_eq!(hex::encode(ret.unwrap().output), case.expected);
        }

        for case in read_test_cases("common_bnmul").iter() {
            let ret = call_contract(
                address,
                &hex::decode(case.input.as_str()).unwrap(),
                BN128_MUL_GAS_COST,
            )
            .unwrap();
            assert_eq!(hex::encode(ret.unwrap().output), case.expected);
        }
    }

    #[test]
    fn test_bn128_pairing() {
        let address = H160([
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x08,
        ]);

        for case in read_test_cases("bn256Pairing").iter() {
            let ret = call_contract(
                address,
                &hex::decode(case.input.as_str()).unwrap(),
                case.gas,
            )
            .unwrap();
            assert_eq!(hex::encode(ret.unwrap().output), case.expected);
        }

        for case in read_test_cases("common_bnpair").iter() {
            let ret = call_contract(
                address,
                &hex::decode(case.input.as_str()).unwrap(),
                BN128_PAIRING_BASE_GAS_COST
                    + BN128_PAIRING_CHECK_GAS_COST * (case.input.len() as u64 / 384),
            )
            .unwrap();
            assert_eq!(hex::encode(ret.unwrap().output), case.expected);
        }
    }
}
