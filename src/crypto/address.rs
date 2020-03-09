use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct H160([u8; 20]);

impl std::convert::From<&[u8]> for H160 {
    fn from(input: &[u8]) -> H160 {
    	let length = input.len();
    	let slice = &input[(length-20)..length];
        let mut buffer: [u8; 20] = [0; 20];
        buffer[..].copy_from_slice(&slice);
        return H160(buffer);
    }
}