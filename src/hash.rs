use sha2::{Digest, Sha256};

pub fn get_hash(input: &str) -> Vec<u8> {
    Sha256::digest(input).to_vec()
}
