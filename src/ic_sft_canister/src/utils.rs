use ciborium::{from_reader, into_writer};
use hmac::{Hmac, Mac};
use serde::Serialize;
use sha3::{Digest, Sha3_256};

// sha3_256 returns the SHA3-256 hash of the input data.
pub fn sha3_256(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha3_256::new();
    hasher.update(data);
    hasher.finalize().into()
}

// mac_256 returns the HMAC-SHA3-256 of the input data using the given key.
pub fn mac_256(key: &[u8], add: &[u8]) -> [u8; 32] {
    let mut mac = Hmac::<Sha3_256>::new_from_slice(key).expect("HMAC can take key of any size");
    mac.update(add);
    mac.finalize().into_bytes().into()
}

// to_cbor_bytes returns the CBOR encoding of the given object that implements the Serialize trait.
pub fn to_cbor_bytes(obj: &impl Serialize) -> Vec<u8> {
    let mut buf: Vec<u8> = Vec::new();
    into_writer(obj, &mut buf).expect("Failed to encode in CBOR format");
    buf
}

// Challenge is a trait for generating and verifying challenges.
pub trait Challenge {
    fn challenge(&self, key: &[u8], timestamp: u64) -> Vec<u8>;
    fn verify(&self, key: &[u8], expire_at: u64, challenge: &[u8]) -> Result<(), String>;
}

// Implement the Challenge trait for any type that implements the Serialize trait.
impl<T> Challenge for T
where
    T: Serialize,
{
    fn challenge(&self, key: &[u8], timestamp: u64) -> Vec<u8> {
        let mac = &mac_256(key, &to_cbor_bytes(self))[0..16];
        to_cbor_bytes(&vec![&to_cbor_bytes(&timestamp), mac])
    }

    fn verify(&self, key: &[u8], expire_at: u64, challenge: &[u8]) -> Result<(), String> {
        let arr: Vec<Vec<u8>> =
            from_reader(challenge).map_err(|_err| "Failed to decode the challenge")?;
        if arr.len() != 2 {
            return Err("Invalid challenge".to_string());
        }

        let timestamp: u64 = from_reader(&arr[0][..])
            .map_err(|_err| "Failed to decode timestamp in the challenge")?;
        if timestamp < expire_at {
            return Err("The challenge is expired".to_string());
        }

        let mac = &mac_256(key, &to_cbor_bytes(self))[0..16];
        if mac != &arr[1][..] {
            return Err("Failed to verify the challenge".to_string());
        }

        Ok(())
    }
}
