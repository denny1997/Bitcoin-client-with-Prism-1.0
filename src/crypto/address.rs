use serde::{Serialize, Deserialize};
use std::fmt;

#[derive(Serialize, Deserialize, Debug, Default, Clone, Eq, PartialEq, Hash, Copy)]
pub struct H160([u8; 20]);

impl std::convert::From<&[u8]> for H160 {
    fn from(input: &[u8]) -> H160 {
        let hash = ring::digest::digest(&ring::digest::SHA256, input).clone();
        let hash_ref = hash.as_ref();
    	let length = hash_ref.len();
    	let slice = &hash_ref[(length-20)..length];
        let mut buffer: [u8; 20] = [0; 20];
        buffer[..].copy_from_slice(&slice);
        return H160(buffer);
    }
}

impl std::fmt::Display for H160 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", std::str::from_utf8(&self.0[..]).unwrap().to_string())
    }
}