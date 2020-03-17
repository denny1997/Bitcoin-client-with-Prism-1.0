use serde::{Serialize,Deserialize};
use ring::signature::{Ed25519KeyPair, Signature, KeyPair, VerificationAlgorithm, EdDSAParameters};
use rand::Rng;
use ring::digest;
use crate::crypto::hash::{H256, Hashable};
use crate::crypto::address::H160;
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct StatePerBlock {
    pub spb: HashMap<H256, State>,
}

impl StatePerBlock {
    pub fn new(genesis_hash: H256, genesis_state: State) -> Self {
        let mut spb: HashMap<H256,State> = HashMap::new();
        spb.insert(genesis_hash, genesis_state);
        return StatePerBlock{spb:spb};
    }

    pub fn insert(&mut self, block_hash: H256, state: &State) {
        self.spb.insert(block_hash,state.clone());
    }
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct State {
    pub states: HashMap<H160,(u32,u32)>,
}

impl State {
    
	pub fn new() -> Self {
	            let mut states: HashMap<H160,(u32,u32)> = HashMap::new();
	            return State{states:states};
	}

    pub fn insert(&mut self, address: H160, balance: u32, nonce: u32) {
        self.states.insert(address,(nonce,balance));
    }

    pub fn addressCheck(&self, public_key: &[u8]) -> bool {
        if self.states.contains_key(&public_key.into()) {
            return true;
        } else {
            return false;
        }
    }

    pub fn spendCheck(&self, public_key: &[u8], value:u32, accountNonce:u32) -> bool {
        let accountInfo = self.states[&public_key.into()];
        if (accountInfo.0 < accountNonce) && (accountInfo.1 >= value) {
        // if (accountInfo.1 >= value){
            return true;
        } else {
            return false;
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Mempool {
    pub transactions: HashMap<H256,SignedTransaction>,
}

impl Mempool {
    pub fn new() -> Self {
        let mut transactions: HashMap<H256,SignedTransaction> = HashMap::new();
        return Mempool{transactions:transactions};
    }

    pub fn insert(&mut self, transaction: &SignedTransaction) {
        self.transactions.insert(transaction.hash(),transaction.clone());
    }
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct SignedTransaction {
    pub signature: Vec<u8>,
    pub public_key: Vec<u8>,
    pub transaction: Transaction,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Transaction {
    pub recipientAddr: H160,
    pub value: u32,
    pub accountNonce: u32,
}

/// Create digital signature of a transaction
pub fn sign(t: &Transaction, key: &Ed25519KeyPair) -> Signature {
    let encoded_struct: Vec<u8> = bincode::serialize(t).unwrap();
    let signature = key.sign(&encoded_struct);
    return signature;
}

/// Verify digital signature of a transaction, using public key instead of secret key
// 
pub fn verify(t: &Transaction, public_key: &[u8], signature: &[u8]) -> bool {
    let encoded_struct: Vec<u8> = bincode::serialize(t).unwrap();
    let peer_public_key = ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519, public_key);
    return peer_public_key.verify(&encoded_struct, signature).is_ok();    
}

impl Hashable for Transaction {
    fn hash(&self) -> H256 {
        let encoded_struct: Vec<u8> = bincode::serialize(&self).unwrap();
        let hashed_struct = digest::digest(&digest::SHA256, &encoded_struct);
        return hashed_struct.into();
    }
}

impl Hashable for SignedTransaction {
    fn hash(&self) -> H256 {
        let encoded_struct: Vec<u8> = bincode::serialize(&self).unwrap();
        let hashed_struct = digest::digest(&digest::SHA256, &encoded_struct);
        return hashed_struct.into();
    }
}

#[cfg(any(test, test_utilities))]
mod tests {
    use super::*;
    use crate::crypto::key_pair;

    pub fn generate_random_transaction() -> Transaction {
        //Default::default();
        let mut rng = rand::thread_rng();
        let n1: u8 = rng.gen();
        Transaction{transaction: n1}
    }

    #[test]
    fn sign_verify() {
        let t = generate_random_transaction();
        let key = key_pair::random();
        let signature = sign(&t, &key);
        assert!(verify(&t, &(key.public_key()), &signature.as_ref()));
    }
}
