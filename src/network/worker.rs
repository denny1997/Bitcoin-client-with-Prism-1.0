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
use crate::transaction::{verify,Mempool,TxBlockMempool,State,StatePerBlock};
use crate::crypto::address::H160;
use log::info;

#[derive(Clone)]
pub struct Context {
    msg_chan: channel::Receiver<(Vec<u8>, peer::Handle)>,
    num_worker: usize,
    server: ServerHandle,
    blockchain: Arc<Mutex<Blockchain>>,
    buffer: Arc<Mutex<HashMap<H256, Block>>>,
    mempool: Arc<Mutex<Mempool>>,
    txBlockmempool: Arc<Mutex<TxBlockMempool>>,
    txBlockOrderedList: Arc<Mutex<Vec<H256>>>,
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
    txBlockmempool: &Arc<Mutex<TxBlockMempool>>, 
    txBlockOrderedList: &Arc<Mutex<Vec<H256>>>, 
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
        txBlockmempool: Arc::clone(txBlockmempool),
        txBlockOrderedList: Arc::clone(txBlockOrderedList),
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
            let temp_txBlockmempool = Arc::clone(&self.txBlockmempool);
            let mut txBlockmempool = temp_txBlockmempool.lock().unwrap();

            let temp_txBlockOrderedList = Arc::clone(&self.txBlockOrderedList);
            let mut txBlockOrderedList = temp_txBlockOrderedList.lock().unwrap();
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
                    info!("Receive one tx");
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

                Message::NewTxBlockHashes(hashes) => {
                    debug!("NewTxBlockHashes");
                    let mut h = vec![];
                    for hash in hashes {
                        if !txBlockmempool.txBlocks.contains_key(&hash) {
                            h.push(hash);
                        }
                    }
                    // let ttt = h.clone();
                    if h.len()>0{
                        // self.server.broadcast(Message::NewBlockHashes(ttt));
                        peer.write(Message::GetTxBlocks(h));
                    }
                }

                Message::GetTxBlocks(hashes) => {
                    // let ttt = hashes.clone();
                    // peer.write(Message::GetBlocks(ttt));
                    debug!("GetTxBlocks");
                    let mut b = vec![];
                    for hash in hashes {
                        if txBlockmempool.txBlocks.contains_key(&hash) {
                            b.push(txBlockmempool.txBlocks[&hash].clone());
                        }
                    }
                    if b.len()>0{
                        peer.write(Message::TxBlocks(b));
                        // println!("Blockchain length: {:?}", blockchain.blocks.len());
                        // println!("Buffer length: {:?}", (*buffer).len());
                        // println!("Tip: {:?}", (*blockchain).tip());
                    }                   
                }

                Message::TxBlocks(blocks) => {
                    debug!("TxBlocks");
                    let mut broadcast_blocks_hashes = vec![];
                    for block in blocks {
                        // println!("1");
                        if block.header.difficultyForTx >= block.hash() {
                            if !txBlockmempool.txBlocks.contains_key(&block.hash()) {
                                if (block.header.difficultyForTx == blockchain.blocks[&block.header.parent].header.difficultyForTx) && (block.header.difficultyForPr == blockchain.blocks[&block.header.parent].header.difficultyForPr) {
                                    let contents = &(&block.clone()).content.data;
                                    let mut flag = false; 

                                    for signedTransaction in contents {
                                        let signature = &signedTransaction.signature;
                                        let public_key = &signedTransaction.public_key;
                                        let transaction = &signedTransaction.transaction;
                                        // Signature check CODE
                                        if !verify(transaction, public_key, signature) {
                                            flag = true;    // invalid signature
                                            break;
                                            println!("ooooooooops, something is not good!");
                                        }

                                    }
                                    if flag {
                                        break;
                                    }

                                    (*txBlockmempool).insert(&block);
                                    (*txBlockOrderedList).push(block.hash());
                                    
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

                                    broadcast_blocks_hashes.push(block.clone().hash());
                                }                        
                            }
                        }                       
                    }

                    if broadcast_blocks_hashes.len() > 0 {
                        self.server.broadcast(Message::NewTxBlockHashes(broadcast_blocks_hashes));
                    }
                    // println!("Blockchain length: {:?}", blockchain.blocks.len());
                    // println!("Buffer length: {:?}", (*buffer).len());
                    // println!("Tip: {:?}", (*blockchain).tip());
                    println!("???????");
                    info!("Tx block received !! Blockchain length: {:?}, Block tip: {:?}", blockchain.blocks.len(), (*blockchain).tip());
                    println!("???????");
                }

                Message::NewPrBlockHashes(hashes) => {
                    debug!("NewPrBlockHashes");
                    let mut h = vec![];
                    for hash in hashes {
                        if !blockchain.blocks.contains_key(&hash) {
                            h.push(hash);
                        }
                    }
                    // let ttt = h.clone();
                    if h.len()>0{
                        // self.server.broadcast(Message::NewBlockHashes(ttt));
                        peer.write(Message::GetPrBlocks(h));
                    }
                }
                Message::GetPrBlocks(hashes) => {
                    // let ttt = hashes.clone();
                    // peer.write(Message::GetBlocks(ttt));
                    debug!("GetPrBlocks");
                    let mut b = vec![];
                    for hash in hashes {
                        if blockchain.blocks.contains_key(&hash) {
                            b.push(blockchain.blocks[&hash].clone());
                        }
                    }
                    if b.len()>0{
                        peer.write(Message::PrBlocks(b));
                        // println!("Blockchain length: {:?}", blockchain.blocks.len());
                        // println!("Buffer length: {:?}", (*buffer).len());
                        // println!("Tip: {:?}", (*blockchain).tip());
                    }
                    
                    
                }
                Message::PrBlocks(blocks) => {
                    // let ttt = blocks.clone();
                    // peer.write(Message::Blocks(ttt));
                    debug!("PrBlocks");
                    info!("Receive one block");
                    let mut p = vec![];
                    let mut broadcast_blocks_hashes = vec![];
                    for block in blocks {
                        // println!("1");
                        if block.header.difficultyForPr >= block.hash() {
                            if !blockchain.blocks.contains_key(&block.hash()) {
                                if !blockchain.blocks.contains_key(&block.header.parent){                                   
                                    (*buffer).insert(block.header.parent,block.clone());
                                    debug!("Parent not recieved yet");
                                    p.push(block.header.parent)                                                                     
                                } else {
                                    if (block.header.difficultyForTx == blockchain.blocks[&block.header.parent].header.difficultyForTx) && (block.header.difficultyForPr == blockchain.blocks[&block.header.parent].header.difficultyForPr) {
                                        let mut flag = false; 
                                        // The state is reverted when a fork becomes the new longest chain. CODE
                                        let mut state = spb.spb[&block.header.parent].clone();
                                        let mut orderList = spb.spb[&block.header.parent].txBlockOrderedList.clone();

                                        let mut validTransaction = vec![];
                                        
                                        let tp = block.txPointer.tp.clone();

                                        for txpointer in tp {
                                            if !orderList.contains(&txpointer) {
                                                state.txBlockOrderedList.push(txpointer);
                                                if txBlockmempool.txBlocks.contains_key(&txpointer) {
                                                    let txBlk = txBlockmempool.txBlocks[&txpointer].clone();
                                                    let contents = txBlk.content.data;

                                                    for signedTransaction in contents {
                                                        let signature = &signedTransaction.signature;
                                                        let public_key = &signedTransaction.public_key;
                                                        let transaction = &signedTransaction.transaction;
                                                        // Signature check CODE
                                                        if !verify(transaction, public_key, signature) {
                                                            flag = true;    // invalid signature
                                                            break;
                                                            println!("ooooooooops, something is not good!");
                                                        }
                                                        let recipientAddr = transaction.recipientAddr;
                                                        let value = transaction.value;
                                                        let accountNonce = transaction.accountNonce;

                                                        let senderAddr: H160 = public_key[..].into();

                                                        if !state.addressCheck(&public_key[..]) {
                                                            state.insert(public_key[..].into(), 1000, 0);
                                                            info!("Offer 1000 coins to address {}", senderAddr);
                                                        }
                                                        if !state.states.contains_key(&recipientAddr){
                                                            state.insert(recipientAddr, 1000, 0);
                                                            info!("Offer 1000 coins to address {}", senderAddr);
                                                        }

                                                        if state.spendCheck(&public_key[..], value, accountNonce) {
                                                            validTransaction.push(signedTransaction);
                                                        }
                
                                                    }
                                                    if flag {
                                                        break;
                                                    }
                                                }
                                                else {
                                                    println!("Fatal error!");
                                                }                                               
                                            }                      
                                        }

                                        if flag {
                                            break;
                                        }                                        
                                      
                                        for vt in validTransaction {
                                            let public_key = &vt.public_key;
                                            let transaction = &vt.transaction;
                                            let recipientAddr = transaction.recipientAddr;
                                            let senderAddr: H160 = public_key[..].into();
                                            let value = transaction.value;
                                            let sender = state.states[&senderAddr];
                                            let repient = state.states[&recipientAddr];
                                            state.insert(senderAddr,(sender.1)-value, (sender.0)+1);
                                            state.insert(recipientAddr,(repient.1)+value, repient.0);
                                            info!("{:} received {:?} coins from {:}", 
                                                recipientAddr,
                                                value,
                                                senderAddr,
                                            );
                                        }

                                        (*spb).insert(block.hash(),&state);
                                        (*blockchain).insert(&block);                                      

                                        // let currentTime = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_millis();
                                        // let durationSinceMined = currentTime - block.header.timestamp;
                                        // println!("!!!!!!!!!!!!!!!!!!!Latency: {:?}", durationSinceMined);
                                        broadcast_blocks_hashes.push(block.clone().hash());
                                        let mut parent = block.hash();
                                        let difficultyForPr = block.header.difficultyForPr;
                                        let difficultyForTx = block.header.difficultyForTx;
                                        let mut temp;
                                        while (*buffer).contains_key(&parent) {
                                            println!("stucked! TAT");
                                            if ((*buffer)[&parent].header.difficultyForPr != difficultyForPr) || ((*buffer)[&parent].header.difficultyForTx != difficultyForTx) {
                                                break;
                                            }
                                            let mut flag = false; 
                                            // The state is reverted when a fork becomes the new longest chain. CODE
                                            let mut state = spb.spb[&parent].clone();
                                            let mut orderList = spb.spb[&parent].txBlockOrderedList.clone();

                                            let mut validTransaction = vec![];
                                            
                                            let tp = &((*buffer)[&parent].clone()).txPointer.tp.clone();

                                            for txpointer in tp {
                                                if !orderList.contains(&txpointer) {
                                                    state.txBlockOrderedList.push(*txpointer);
                                                    if txBlockmempool.txBlocks.contains_key(&txpointer) {
                                                        let txBlk = txBlockmempool.txBlocks[&txpointer].clone();
                                                        let contents = txBlk.content.data;

                                                        for signedTransaction in contents {
                                                            let signature = &signedTransaction.signature;
                                                            let public_key = &signedTransaction.public_key;
                                                            let transaction = &signedTransaction.transaction;
                                                            // Signature check CODE
                                                            if !verify(transaction, public_key, signature) {
                                                                flag = true;    // invalid signature
                                                                break;
                                                                println!("ooooooooops, something is not good!");
                                                            }
                                                            let recipientAddr = transaction.recipientAddr;
                                                            let value = transaction.value;
                                                            let accountNonce = transaction.accountNonce;

                                                            let senderAddr: H160 = public_key[..].into();

                                                            if !state.addressCheck(&public_key[..]) {
                                                                state.insert(public_key[..].into(), 1000, 0);
                                                                info!("Offer 1000 coins to address {}", senderAddr);
                                                            }
                                                            if !state.states.contains_key(&recipientAddr){
                                                                state.insert(recipientAddr, 1000, 0);
                                                                info!("Offer 1000 coins to address {}", senderAddr);
                                                            }

                                                            if state.spendCheck(&public_key[..], value, accountNonce) {
                                                                validTransaction.push(signedTransaction);
                                                            }
                    
                                                        }
                                                        if flag {
                                                            break;
                                                        }
                                                    }
                                                    else {
                                                        println!("Fatal error!");
                                                    }                                               
                                                }                      
                                            }

                                            if flag {
                                                break;
                                            }                                        
                                        
                                            for vt in validTransaction {
                                                let public_key = &vt.public_key;
                                                let transaction = &vt.transaction;
                                                let recipientAddr = transaction.recipientAddr;
                                                let senderAddr: H160 = public_key[..].into();
                                                let value = transaction.value;
                                                let sender = state.states[&senderAddr];
                                                let repient = state.states[&recipientAddr];
                                                state.insert(senderAddr,(sender.1)-value, (sender.0)+1);
                                                state.insert(recipientAddr,(repient.1)+value, repient.0);
                                                info!("{:} received {:?} coins from {:}", 
                                                    recipientAddr,
                                                    value,
                                                    senderAddr,
                                                );
                                            }
          
                                            (*spb).insert((*buffer)[&parent].hash(),&state);
                                            (*blockchain).insert(&(*buffer)[&parent]); 
                               
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

                    if broadcast_blocks_hashes.len() > 0 {
                        self.server.broadcast(Message::NewPrBlockHashes(broadcast_blocks_hashes));
                    }
                    // println!("Blockchain length: {:?}", blockchain.blocks.len());
                    // println!("Buffer length: {:?}", (*buffer).len());
                    // println!("Tip: {:?}", (*blockchain).tip());
                    println!("!!!!!!!!");
                    info!("Pr block received !! Blockchain length: {:?}, Block tip: {:?}", blockchain.blocks.len(), (*blockchain).tip());
                    println!("!!!!!!!!");
                }
            }
        }
    }
}
