use serde::{Serialize,Deserialize};
use ring::signature::{Ed25519KeyPair, Signature, KeyPair, VerificationAlgorithm, EdDSAParameters};
use rand::Rng;
use ring::digest;
use crate::crypto::hash::{H256, Hashable};

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Transaction {
    transaction: u8,
}

/// Create digital signature of a transaction
pub fn sign(t: &Transaction, key: &Ed25519KeyPair) -> Signature {
    let encoded_struct: Vec<u8> = bincode::serialize(t).unwrap();
    let signature = key.sign(&encoded_struct);
    return signature;
}

/// Verify digital signature of a transaction, using public key instead of secret key
pub fn verify(t: &Transaction, public_key: &<Ed25519KeyPair as KeyPair>::PublicKey, signature: &Signature) -> bool {
    let encoded_struct: Vec<u8> = bincode::serialize(t).unwrap();
    let peer_public_key = ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519, public_key);
    return peer_public_key.verify(&encoded_struct, signature.as_ref()).is_ok();    
}

impl Hashable for Transaction {
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
        assert!(verify(&t, &(key.public_key()), &signature));
    }
}
