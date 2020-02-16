use serde::{Serialize, Deserialize};
use crate::crypto::hash::{H256, Hashable};
use crate::transaction::{Transaction};
use ring::digest;
use rand::Rng;
use crate::crypto::merkle::{MerkleTree};

#[derive(Serialize, Deserialize, Debug)]
pub struct Header {
    pub parent:H256,
    pub nonce:u32,
    pub difficulty:H256,
    pub timestamp:u128,
    pub merkle_root:H256
}

impl Hashable for Header {
    fn hash(&self) -> H256 {
        let encoded_struct: Vec<u8> = bincode::serialize(&self).unwrap();
        let hashed_struct = digest::digest(&digest::SHA256, &encoded_struct);
        return hashed_struct.into();
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Content {
    pub data:Vec<Transaction>
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Block {
    pub header:Header,
    pub content:Content,
}

impl Hashable for Block {
    fn hash(&self) -> H256 {
        return self.header.hash();
    }
}

#[cfg(any(test, test_utilities))]
pub mod test {
    use super::*;
    use crate::crypto::hash::H256;

    pub fn generate_random_block(parent: &H256) -> Block {
        let mut rng = rand::thread_rng();
        let n1: u32 = rng.gen();
        let n2: u64 = rng.gen();
        let data = vec![];
        let merkle_root = MerkleTree::new(&data).root();
        let header:Header = Header{parent:*parent,nonce:n1,difficulty:*parent,timestamp:n2,merkle_root:merkle_root};
        let content:Content = Content{data:data};
        let block: Block = Block{header: header, content: content};
        return block;
    }
}
