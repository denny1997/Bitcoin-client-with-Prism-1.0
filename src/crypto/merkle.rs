use super::hash::{Hashable, H256};
use ring::digest;

/// A Merkle tree.
#[derive(Debug, Default)]
pub struct MerkleTree {
    hash_idx: Vec<H256>,
}

impl MerkleTree {
    pub fn new<T>(data: &[T]) -> Self where T: Hashable, {
        let mut hash_index = vec![];
        let mut num = data.len();
        let mut base = 0;
        for i in 0..num {
            hash_index.push(data[i].hash());
        }
        let mut p = 0;
        let n:usize = 2;
        while num > 1 {
            if num % 2 == 1 {
                let len = hash_index.len();
                for k in len-n.pow(p)..len {
                    hash_index.push(hash_index[k]);
                num += 1;
                }
            }
            num /= 2;
            p += 1;
        }
        num = hash_index.len();
        while num > 1 {
            for i in 0..num/2 {
                let mut ctx = digest::Context::new(&digest::SHA256);
                ctx.update(&<[u8;32]>::from(&hash_index[base+i*2]));
                ctx.update(&<[u8;32]>::from(&hash_index[base+i*2+1]));
                hash_index.push((ctx.finish()).into());
            }
            base = base + num;
            num = num/2;
            if num % 2 > 0 && num > 1 {
                hash_index.push(hash_index[base+num-1]);
                num = num + 1;
            }
        }

        hash_index.reverse();
        return MerkleTree{hash_idx:hash_index};
    }

    pub fn root(&self) -> H256 {
        if self.hash_idx.len() == 0{
            return hex!("0000000000000000000000000000000000000000000000000000000000000000").into();
        }
        return self.hash_idx[0];
    }
    /// Returns the Merkle Proof of data at index i
    pub fn proof(&self, index: usize) -> Vec<H256> {
        let mut res = vec![];
        let len = self.hash_idx.len();
        let mut pos = len - index;
        while pos > 1 {
            if pos % 2 == 0 {
                res.push(self.hash_idx[pos]);
            }
            else {
                res.push(self.hash_idx[pos-2]);
            }
            pos = pos / 2;
        }
        
        return res;
    }
}

/// Verify that the datum hash with a vector of proofs will produce the Merkle root. Also need the
/// index of datum and `leaf_size`, the total number of leaves.
pub fn verify(root: &H256, datum: &H256, proof: &[H256], index: usize, leaf_size: usize) -> bool {
    let mut p = 1;
    let n:usize = 2;
    while n.pow(p)<leaf_size {
        p+=1;
    }
    let mut len = n.pow(p);
    let mut level = len/2;
    while level > 0 {
        len = len + level;
        level = level / 2;
    }
    let mut pos = len - index;
    let mut i = 0;
    let mut res: H256 = *datum;
    while pos > 1 {
        if pos % 2 ==0 {
            let mut ctx = digest::Context::new(&digest::SHA256);
            ctx.update(&<[u8;32]>::from(&proof[i]));
            ctx.update(&<[u8;32]>::from(&res));
            res = (ctx.finish()).into();
            i = i + 1;             
        }
        else {
            let mut ctx = digest::Context::new(&digest::SHA256);
            ctx.update(&<[u8;32]>::from(&res));
            ctx.update(&<[u8;32]>::from(&proof[i]));            
            res = (ctx.finish()).into();
            i = i + 1;
        }
        pos = pos / 2;
    }

    if res == *root {
        return true;
    }
    else {
        return false;
    }
}

#[cfg(test)]
mod tests {
    use crate::crypto::hash::H256;
    use super::*;

    macro_rules! gen_merkle_tree_data {
        () => {{
            vec![
                (hex!("0a0b0c0d0e0f0e0d0a0b0c0d0e0f0e0d0a0b0c0d0e0f0e0d0a0b0c0d0e0f0e0d")).into(),
                (hex!("0101010101010101010101010101010101010101010101010101010101010202")).into(),
                (hex!("0a0b0c0d0e0f0e0d0a0b0c0d0e0f0e0d0a0b0c0d0e0f0e0d0a0b0c0d0e0f0e0d")).into(),
                (hex!("0a0b0c0d0e0f0e0d0a0b0c0d0e0f0e0d0a0b0c0d0e0f0e0d0a0b0c0d0e0f0e0d")).into(),
                (hex!("0a0b0c0d0e0f0e0d0a0b0c0d0e0f0e0d0a0b0c0d0e0f0e0d0a0b0c0d0e0f0e0d")).into(),
            ]
        }};
    }

    #[test]
    fn root() {
        let input_data: Vec<H256> = gen_merkle_tree_data!();
        let merkle_tree = MerkleTree::new(&input_data);
        let root = merkle_tree.root();
        assert_eq!(
            root,
            //(hex!("6b787718210e0b3b608814e04e61fde06d0df794319a12162f287412df3ec920")).into()
            (hex!("f0de10bbc0838878b0c4175d9a9d900dc5fc26dfbc0bc182e629d197b3b32c5f")).into()
        );
        // "b69566be6e1720872f73651d1851a0eae0060a132cf0f64a0ffaea248de6cba0" is the hash of
        // "0a0b0c0d0e0f0e0d0a0b0c0d0e0f0e0d0a0b0c0d0e0f0e0d0a0b0c0d0e0f0e0d"
        // "965b093a75a75895a351786dd7a188515173f6928a8af8c9baa4dcff268a4f0f" is the hash of
        // "0101010101010101010101010101010101010101010101010101010101010202"
        // "6b787718210e0b3b608814e04e61fde06d0df794319a12162f287412df3ec920" is the hash of
        // the concatenation of these two hashes "b69..." and "965..."
        // notice that the order of these two matters
    }

    #[test]
    fn proof() {
        let input_data: Vec<H256> = gen_merkle_tree_data!();
        let merkle_tree = MerkleTree::new(&input_data);
        let proof = merkle_tree.proof(0);
        assert_eq!(proof,
                   vec![hex!("965b093a75a75895a351786dd7a188515173f6928a8af8c9baa4dcff268a4f0f").into(),
                        hex!("8c56ff4c190d4f6cd98b87661e77da02ce4c1436de294382278bfb915c30576c").into(),
                        hex!("7e85800ae9e1754abd05983bda78efa0834a16163927bef710cd7f49e315b58b").into()]                
        );
        // "965b093a75a75895a351786dd7a188515173f6928a8af8c9baa4dcff268a4f0f" is the hash of
        // "0101010101010101010101010101010101010101010101010101010101010202"
    }

    #[test]
    fn verifying() {
        let input_data: Vec<H256> = gen_merkle_tree_data!();
        let merkle_tree = MerkleTree::new(&input_data);
        let proof = merkle_tree.proof(0);
        assert!(verify(&merkle_tree.root(), &input_data[0].hash(), &proof, 0, input_data.len()));
    }
}
