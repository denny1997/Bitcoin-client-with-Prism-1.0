use crate::block::{Block,Header,Content};
use crate::crypto::hash::H256;
use std::collections::HashMap;
use crate::crypto::merkle::{MerkleTree};
use crate::crypto::hash::Hashable;

#[derive(Debug, Default, Clone)]
pub struct Blockchain {
    pub blocks:HashMap<H256, Block>,
    pub height:HashMap<H256, u32>,
    pub last_block_of_longest_chain: H256,
    pub genesis:H256
}

impl Blockchain {
    /// Create a new blockchain, only containing the genesis block
    pub fn new() -> Self {
        let mut blocks: HashMap<H256,Block> = HashMap::new();
        let mut height: HashMap<H256,u32> = HashMap::new();
        let data = vec![];
        let merkle_root = MerkleTree::new(&data).root();
        // let t1 = [255; 16];
        // let t2 = [0; 8];
        let mut difficulty: [u8; 32] = [0,0,127,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255];
        // difficulty.copy_from_slice(&([t2,t1].concat())[..]);
        let header:Header = Header{parent:merkle_root,nonce:0,difficulty:difficulty.into(),timestamp:0,merkle_root:merkle_root};
        let content:Content = Content{data:data};
        let genesis: Block = Block{header: header, content: content};
        let hash = genesis.hash();
        blocks.insert(hash,genesis);
        height.insert(hash,0);
        return Blockchain{blocks:blocks,height:height,last_block_of_longest_chain:hash,genesis:hash};
    }

    /// Insert a block into blockchain
    pub fn insert(&mut self, block: &Block) {
        let last = block.header.parent;
        let h = self.height[&last];
        self.blocks.insert(block.hash(),Block{header:Header{parent:last,nonce:block.header.nonce,difficulty:block.header.difficulty,timestamp:block.header.timestamp,merkle_root:block.header.merkle_root},content:Content{data:(&block.content.data).to_vec()}});
        self.height.insert(block.hash(),h+1);
        if h+1 > self.height[&self.last_block_of_longest_chain] {
            self.last_block_of_longest_chain = block.hash();
        }
    }

    /// Get the last block's hash of the longest chain
    pub fn tip(&self) -> H256 {
        return self.last_block_of_longest_chain;
    }

    /// Get the last block's hash of the longest chain
    #[cfg(any(test, test_utilities))]
    pub fn all_blocks_in_longest_chain(&self) -> Vec<H256> {
        let mut blocks = vec![];
        let mut curBlock = self.last_block_of_longest_chain;
        while curBlock != self.genesis {
            blocks.push(curBlock);
            curBlock = self.blocks[&curBlock].header.parent;
        }
        blocks.push(curBlock);
        blocks.reverse();
        return blocks;
    }
}

#[cfg(any(test, test_utilities))]
mod tests {
    use super::*;
    use crate::block::test::generate_random_block;
    use crate::crypto::hash::Hashable;

    #[test]
    fn insert_one() {
        let mut blockchain = Blockchain::new();
        let genesis_hash = blockchain.tip();
        let block = generate_random_block(&genesis_hash);
        blockchain.insert(&block);
        assert_eq!(blockchain.tip(), block.hash());

    }
}
