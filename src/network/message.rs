use serde::{Serialize, Deserialize};
use crate::crypto::hash::H256;
use crate::block::Block;
use crate::transaction::SignedTransaction;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Message {
    Ping(String),
    Pong(String),
    NewPrBlockHashes(Vec<H256>),
    GetPrBlocks(Vec<H256>),
    PrBlocks(Vec<Block>),
    NewTxBlockHashes(Vec<H256>),
    GetTxBlocks(Vec<H256>),
    TxBlocks(Vec<Block>),
    NewTransactionHashes(Vec<H256>),
    GetTransactions(Vec<H256>),
    Transactions(Vec<SignedTransaction>),
}
