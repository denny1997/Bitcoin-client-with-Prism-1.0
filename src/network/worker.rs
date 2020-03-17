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
use crate::transaction::{verify,Mempool,State,StatePerBlock};
use crate::crypto::address::H160;

#[derive(Clone)]
pub struct Context {
    msg_chan: channel::Receiver<(Vec<u8>, peer::Handle)>,
    num_worker: usize,
    server: ServerHandle,
    blockchain: Arc<Mutex<Blockchain>>,
    buffer: Arc<Mutex<HashMap<H256, Block>>>,
    mempool: Arc<Mutex<Mempool>>,
    // state: Arc<Mutex<State>>,
    spb: Arc<Mutex<StatePerBlock>>,
}

pub fn new(
    num_worker: usize,
    msg_src: channel::Receiver<(Vec<u8>, peer::Handle)>,
    server: &ServerHandle,
    blockchain: &Arc<Mutex<Blockchain>>,
    hashMap: &Arc<Mutex<HashMap<H256, Block>>>,
    mempool: &Arc<Mutex<Mempool>>,
    // state: &Arc<Mutex<State>>,
    spb: &Arc<Mutex<StatePerBlock>>,
) -> Context {
    Context {
        msg_chan: msg_src,
        num_worker,
        server: server.clone(),
        blockchain: Arc::clone(blockchain),
        buffer: Arc::clone(hashMap),
        mempool: Arc::clone(mempool),
        // state: Arc::clone(state),
        spb: Arc::clone(spb),
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

            // let temp_state = Arc::clone(&self.state);
            // let mut state = temp_state.lock().unwrap();

            let temp_spb = Arc::clone(&self.spb);
            let mut spb = temp_spb.lock().unwrap();

            let temp_buffer = Arc::clone(&self.buffer);
            let mut buffer = temp_buffer.lock().unwrap();
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
                    // println!("1");
                    // println!("{:?}", blockchain.tip());
                    let state = &spb.spb[&blockchain.tip()];
                    // println!("2");
                    for transaction in transactions {
                        if !mempool.transactions.contains_key(&transaction.hash()) {                           
                            let signature = &transaction.signature;
                            let public_key = &transaction.public_key;
                            let transaction_content = &transaction.transaction;
                            if verify(transaction_content, public_key, signature) {
                                if !state.addressCheck(public_key) {
                                    // (*state).insert(public_key[..].into(), 1000, 0);
                                    // if (transaction_content.value <= 1000) && (transaction_content.accountNonce == 1){
                                    if (transaction_content.value <= 1000){
                                        (*mempool).insert(&transaction);
                                        broadcast_transactions_hashes.push(transaction.clone().hash());
                                    }
                                    else{
                                        println!("not add 1");
                                    }
                                } else {
                                    if state.spendCheck(public_key, transaction_content.value, transaction_content.accountNonce) {
                                        (*mempool).insert(&transaction);
                                        broadcast_transactions_hashes.push(transaction.clone().hash());
                                    } 
                                    else{
                                        println!("not add 2");
                                    }
                                }
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
                        println!("Buffer length: {:?}", (*buffer).len());
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
                        // println!("1");
                        if block.header.difficulty >= block.hash() {
                            if !blockchain.blocks.contains_key(&block.hash()) {
                                if !blockchain.blocks.contains_key(&block.header.parent){                                   
                                    (*buffer).insert(block.header.parent,block.clone());
                                    debug!("Parent not recieved yet");
                                    p.push(block.header.parent)                                                                     
                                } else {
                                    if block.header.difficulty == blockchain.blocks[&block.header.parent].header.difficulty {
                                        let contents = &(&block.clone()).content.data;
                                        let mut flag = false; 
                                        let mut state = spb.spb[&block.header.parent].clone();

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
                                                println!("ooooooooops, something is not good!");
                                            }
                                            // println!("2");
                                            let recipientAddr = transaction.recipientAddr;
                                            let value = transaction.value;
                                            let accountNonce = transaction.accountNonce;
                                            // println!("21");
                                            let senderAddr: H160 = public_key[..].into();
                                            // println!("22");
                                            if !state.addressCheck(&public_key[..]) {
                                                state.insert(public_key[..].into(), 1000, 0);
                                            }
                                            if !state.states.contains_key(&recipientAddr){
                                                state.insert(recipientAddr, 1000, 0);
                                            }
                                            // println!("23");
                                            if state.spendCheck(&public_key[..], value, accountNonce) {
                                                // println!("231");
                                                let sender = state.states[&senderAddr];
                                                // println!("2311");
                                                let repient = state.states[&recipientAddr];
                                                // println!("232");
                                                state.insert(senderAddr,(sender.1)-value, (sender.0)+1);
                                                state.insert(recipientAddr,(repient.1)+value, repient.0);
                                            }
                                            // println!("24");

                                        }
                                        if flag {
                                            break;
                                        }
                                        // println!("3");
                                        // println!("{:?} hhh", block.hash());
                                        (*spb).insert(block.hash(),&state);
                                        // println!("123");
                                        (*blockchain).insert(&block);
                                        // println!("4");
                                        for t in contents{
                                            let key = t.hash();
                                            if (*mempool).transactions.contains_key(&key){
                                                (*mempool).transactions.remove(&key);
                                            }
                                        }
                                        // println!("5");
                                        for (key, value) in (*mempool).transactions.clone().iter() {
                                            for t in contents {
                                                if (t.public_key == value.public_key) && (t.transaction.accountNonce == value.transaction.accountNonce) {
                                                    (*mempool).transactions.remove(&key);
                                                }
                                            }
                                        }
                                        // println!("6");


                                        let currentTime = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_millis();
                                        let durationSinceMined = currentTime - block.header.timestamp;
                                        println!("!!!!!!!!!!!!!!!!!!!Latency: {:?}", durationSinceMined);
                                        broadcast_blocks_hashes.push(block.clone().hash());
                                        let mut parent = block.hash();
                                        let difficulty = block.header.difficulty;
                                        let mut temp;
                                        while (*buffer).contains_key(&parent) {
                                            println!("stucked! TAT");
                                            if (*buffer)[&parent].header.difficulty != difficulty {
                                                break;
                                            }
                                            let contents = &((*buffer)[&parent].clone()).content.data;
                                            let mut flag = false;
                                            let mut state = spb.spb[&parent].clone();
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
                                                    println!("ooooooooops, something is not good!");
                                                    break;
                                                }
                                                let recipientAddr = transaction.recipientAddr;
                                                let value = transaction.value;
                                                let accountNonce = transaction.accountNonce;
                                                let senderAddr: H160 = public_key[..].into();
                                                if !state.addressCheck(&public_key[..]) {
                                                    state.insert(public_key[..].into(), 1000, 0);
                                                }
                                                if state.spendCheck(&public_key[..], value, accountNonce) {
                                                    let sender = state.states[&senderAddr];
                                                    let repient = state.states[&recipientAddr];
                                                    state.insert(senderAddr,(sender.1)-value, (sender.0)+1,);
                                                    state.insert(recipientAddr,(repient.1)+value, repient.0);
                                                }
                                            }
                                            if flag {
                                                break;
                                            }
          
                                            (*spb).insert((*buffer)[&parent].hash(),&state);
                                            (*blockchain).insert(&(*buffer)[&parent]); 

                                            for t in contents{
                                                let key = t.hash();
                                                if (*mempool).transactions.contains_key(&key){
                                                    (*mempool).transactions.remove(&key);
                                                }
                                            }

                                            for (key, value) in (*mempool).transactions.clone().iter() {
                                                for t in contents {
                                                    if (t.public_key == value.public_key) && (t.transaction.accountNonce == value.transaction.accountNonce) {
                                                        (*mempool).transactions.remove(&key);
                                                    }
                                                }
                                            }
    

                                            broadcast_blocks_hashes.push(((*buffer)[&parent]).clone().hash());                         
                                            temp = (*buffer)[&parent].hash();
                                            (*buffer).remove(&parent);
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
                    println!("Buffer length: {:?}", (*buffer).len());
                    println!("Tip: {:?}", (*blockchain).tip());
                }
            }
        }
    }
}
