use super::message::Message;
use super::peer;
use crate::network::server::Handle as ServerHandle;
use crossbeam::channel;
use log::{debug, warn};
use std::sync::{Arc, Mutex};
use crate::blockchain::Blockchain;
use crate::block::Block;
use crate::crypto::hash::{Hashable,H256};
use std::collections::HashMap;
use std::time::SystemTime;
use std::thread;
use ring::signature::{Signature, KeyPair, Ed25519KeyPair};
use crate::transaction::{verify,Mempool};

#[derive(Clone)]
pub struct Context {
    msg_chan: channel::Receiver<(Vec<u8>, peer::Handle)>,
    num_worker: usize,
    server: ServerHandle,
    blockchain: Arc<Mutex<Blockchain>>,
    buffer: HashMap<H256, Block>,
    mempool: Arc<Mutex<Mempool>>,
}

pub fn new(
    num_worker: usize,
    msg_src: channel::Receiver<(Vec<u8>, peer::Handle)>,
    server: &ServerHandle,
    blockchain: &Arc<Mutex<Blockchain>>,
    mempool: &Arc<Mutex<Mempool>>,
) -> Context {
    Context {
        msg_chan: msg_src,
        num_worker,
        server: server.clone(),
        blockchain: Arc::clone(blockchain),
        buffer: HashMap::new(),
        mempool: Arc::clone(mempool),
    }
}

impl Context {
    pub fn start(self) {
        let num_worker = self.num_worker;
        for i in 0..num_worker {
            let mut cloned = self.clone();
            thread::spawn(move || {
                cloned.worker_loop();
                warn!("Worker thread {} exited", i);
            });
        }
    }

    fn worker_loop(&mut self) {
        loop {
            // println!("0");
            let msg = self.msg_chan.recv().unwrap();
            // println!("1");
            let temp = Arc::clone(&self.blockchain);
            // println!("2");
            let mut blockchain = temp.lock().unwrap();
            // println!("3");
            let temp_mempool = Arc::clone(&self.mempool);
            let mut mempool = temp_mempool.lock().unwrap();
            // println!("4");
            let (msg, peer) = msg;
            let msg: Message = bincode::deserialize(&msg).unwrap();
            match msg {
                Message::Ping(nonce) => {
                    debug!("Ping: {}", nonce);
                    peer.write(Message::Pong(nonce.to_string()));
                }
                Message::Pong(nonce) => {
                    debug!("Pong: {}", nonce);
                }
                Message::NewTransactionHashes(hashes) => {
                    debug!("NewTransactionHashes");
                    let mut h = vec![];
                    for hash in hashes {
                        if !mempool.transactions.contains_key(&hash) {
                            h.push(hash);
                        }
                    }
                    if h.len()>0{
                        peer.write(Message::GetTransactions(h));
                    }
                }
                Message::GetTransactions(hashes) => {
                    debug!("GetTransactions");
                    let mut b = vec![];
                    for hash in hashes {
                        if mempool.transactions.contains_key(&hash) {
                            b.push(mempool.transactions[&hash].clone());
                        }
                    }
                    if b.len()>0{
                        peer.write(Message::Transactions(b));
                    }                   
                }
                Message::Transactions(transactions) => {
                    debug!("Transactions");
                    let mut broadcast_transactions_hashes = vec![];
                    for transaction in transactions {
                        if !mempool.transactions.contains_key(&transaction.hash()) {                           
                            let signature = &transaction.signature;
                            let public_key = &transaction.public_key;
                            let transaction_content = &transaction.transaction;
                            if verify(transaction_content, public_key, signature) {
                                (*mempool).insert(&transaction);
                                broadcast_transactions_hashes.push(transaction.clone().hash());
                            }                                                        
                        }                                              
                    }
                    if broadcast_transactions_hashes.len() > 0 {
                        self.server.broadcast(Message::NewTransactionHashes(broadcast_transactions_hashes));
                    }
                }
                Message::NewBlockHashes(hashes) => {
                    debug!("NewBlockHashes");
                    let mut h = vec![];
                    for hash in hashes {
                        if !blockchain.blocks.contains_key(&hash) {
                            h.push(hash);
                        }
                    }
                    // let ttt = h.clone();
                    if h.len()>0{
                        // self.server.broadcast(Message::NewBlockHashes(ttt));
                        peer.write(Message::GetBlocks(h));
                    }
                }
                Message::GetBlocks(hashes) => {
                    // let ttt = hashes.clone();
                    // peer.write(Message::GetBlocks(ttt));
                    debug!("GetBlocks");
                    let mut b = vec![];
                    for hash in hashes {
                        if blockchain.blocks.contains_key(&hash) {
                            b.push(blockchain.blocks[&hash].clone());
                        }
                    }
                    if b.len()>0{
                        peer.write(Message::Blocks(b));
                        println!("Blockchain length: {:?}", blockchain.blocks.len());
                        println!("Buffer length: {:?}", self.buffer.len());
                        println!("Tip: {:?}", (*blockchain).tip());
                    }
                    
                    
                }
                Message::Blocks(blocks) => {
                    // let ttt = blocks.clone();
                    // peer.write(Message::Blocks(ttt));
                    debug!("Blocks");
                    let mut p = vec![];
                    let mut broadcast_blocks_hashes = vec![];
                    for block in blocks {
                        if block.header.difficulty >= block.hash() {
                            if !blockchain.blocks.contains_key(&block.hash()) {
                                if !blockchain.blocks.contains_key(&block.header.parent){                                   
                                    self.buffer.insert(block.header.parent,block.clone());
                                    debug!("Parent not recieved yet");
                                    p.push(block.header.parent)                                                                     
                                } else {
                                    if block.header.difficulty == blockchain.blocks[&block.header.parent].header.difficulty {
                                        let contents = &(&block.clone()).content.data;
                                        let mut flag = false;
                                        for signedTransaction in contents {
                                            // let signature: Signature = bincode::deserialize(&signedTransaction.signature[..]).unwrap();
                                            // let public_key = ring::signature::UnparsedPublicKey::new(&signature::ED25519, signedTransaction.public_key);
                                            // bincode::deserialize(&signedTransaction.public_key[..]).unwrap();
                                            // let transaction = signedTransaction.transaction;
                                            let signature = &signedTransaction.signature;
                                            let public_key = &signedTransaction.public_key;
                                            let transaction = &signedTransaction.transaction;
                                            if !verify(transaction, public_key, signature) {
                                                flag = true;    // invalid signature
                                                break;
                                            }
                                        }
                                        if flag {
                                            break;
                                        }
                                        (*blockchain).insert(&block);
                                        let currentTime = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_millis();
                                        let durationSinceMined = currentTime - block.header.timestamp;
                                        println!("!!!!!!!!!!!!!!!!!!!Latency: {:?}", durationSinceMined);
                                        broadcast_blocks_hashes.push(block.clone().hash());
                                        let mut parent = block.hash();
                                        let difficulty = block.header.difficulty;
                                        let mut temp;
                                        while self.buffer.contains_key(&parent) {
                                            println!("stucked! TAT");
                                            if self.buffer[&parent].header.difficulty != difficulty {
                                                break;
                                            }
                                            let contents = &(self.buffer[&parent].clone()).content.data;
                                            let mut flag = false;
                                            for signedTransaction in contents {
                                                // let signature: Signature = bincode::deserialize(&signedTransaction.signature[..]).unwrap();
                                                // let public_key = ring::signature::UnparsedPublicKey::new(&signature::ED25519, signedTransaction.public_key);
                                                // bincode::deserialize(&signedTransaction.public_key[..]).unwrap();
                                                // let transaction = signedTransaction.transaction;
                                                let signature = &signedTransaction.signature;
                                                let public_key = &signedTransaction.public_key;
                                                let transaction = &signedTransaction.transaction;
                                                if !verify(transaction, public_key, signature) {
                                                    flag = true;    // invalid signature
                                                    break;
                                                }
                                            }
                                            if flag {
                                                break;
                                            }

                                            (*blockchain).insert(&self.buffer[&parent]); 
                                            broadcast_blocks_hashes.push((self.buffer[&parent]).clone().hash());                         
                                            temp = self.buffer[&parent].hash();
                                            self.buffer.remove(&parent);
                                            parent = temp;
                                        }
                                    }
                                }                           
                            }
                        }                       
                    }
                    // if p.len() > 0 {
                    //     peer.write(Message::GetBlocks(p));
                    // }
                    if broadcast_blocks_hashes.len() > 0 {
                        self.server.broadcast(Message::NewBlockHashes(broadcast_blocks_hashes));
                    }
                    println!("Blockchain length: {:?}", blockchain.blocks.len());
                    println!("Buffer length: {:?}", self.buffer.len());
                    println!("Tip: {:?}", (*blockchain).tip());
                }
            }
        }
    }
}
