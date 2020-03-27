use super::message::Message;
use super::peer;
use crate::network::server::Handle as ServerHandle;
use crossbeam::channel;
use log::{debug, warn, info};
use std::sync::{Arc, Mutex};
use crate::blockchain::Blockchain;
use crate::block::Block;
use crate::crypto::hash::{Hashable,H256};
use std::collections::HashMap;
use std::time::SystemTime;
use std::thread;
use ring::signature::{Signature, KeyPair, Ed25519KeyPair};
use crate::transaction::{verify,sign,Mempool,Transaction,SignedTransaction,State};
use rand::Rng;
use crate::crypto::key_pair;
use std::time;
use std::cmp;
use crate::crypto::address::H160;



#[derive(Clone)]
pub struct Context {
    // msg_chan: channel::Receiver<(Vec<u8>, peer::Handle)>,
    num_worker: usize,
    server: ServerHandle,
    // blockchain: Arc<Mutex<Blockchain>>,
    // buffer: HashMap<H256, Block>,
    mempool: Arc<Mutex<Mempool>>,
    // keyPairs: Vec<Ed25519KeyPair>,
    state: Arc<Mutex<State>>,
    key_set: Arc<Mutex<HashMap<u32, Ed25519KeyPair>>>,
}

pub fn new(
    num_worker: usize,
    // msg_src: channel::Receiver<(Vec<u8>, peer::Handle)>,
    server: &ServerHandle,
    // blockchain: &Arc<Mutex<Blockchain>>,
    mempool: &Arc<Mutex<Mempool>>,
    // keyPairs: Vec<Ed25519KeyPair>,
    state: &Arc<Mutex<State>>,
    key_set: &Arc<Mutex<HashMap<u32, Ed25519KeyPair>>>,
) -> Context {
    Context {
        // msg_chan: msg_src,
        num_worker,
        server: server.clone(),
        // blockchain: Arc::clone(blockchain),
        // buffer: HashMap::new(),
        mempool: Arc::clone(mempool),
        // keyPairs: keyPairs.clone(),
        state: Arc::clone(state),
        key_set: Arc::clone(key_set),
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
        let mut record: HashMap<u32, (u32, u32)> = HashMap::new();
        for i in 0..5 {
            record.insert(i,(0,1000));
        }
        let duration = time::Duration::from_millis(5000);
        thread::sleep(duration);
        println!("Start Generator");
        loop {
            let duration = time::Duration::from_millis(2000);
            thread::sleep(duration);
            // println!("I'm running!");
            
            let accountNum = 5;
            let mut rng = rand::thread_rng();
            let senderIdx = rng.gen_range(0,accountNum) as u32;
            let mut recipientIdx = rng.gen_range(0,accountNum) as u32;
            while recipientIdx == senderIdx{
                recipientIdx = rng.gen_range(0,accountNum) as u32;
            }

            let temp_key_set = Arc::clone(&self.key_set);
            let mut key_set = temp_key_set.lock().unwrap();

            let senderKeyPair = &key_set[&senderIdx];
            let senderPublicKey = senderKeyPair.public_key().as_ref();
            let senderPublicKey_hash_h160:H160 = senderPublicKey.into();

            let recipientKeyPair = &key_set[&recipientIdx];
            let recipientNonce = record[&recipientIdx].0;
            let recipientBalance = record[&recipientIdx].1;
            let recipientPublicKey = (key_set[&recipientIdx]).public_key().as_ref();
            let recipientPublicKey_hash_h160:H160 = recipientPublicKey.into();

            let mut currentBalance:u32 = 0;
            let mut nonce:u32 = 0;

            currentBalance = record[&senderIdx].1;
            nonce = &record[&senderIdx].0+1;
            

            // let temp_state = Arc::clone(&self.state);
            // let mut state = temp_state.lock().unwrap();
            
            // if state.states.contains_key(&senderPublicKey_hash_h160) {
            //     maxvalue = state.states[&senderPublicKey_hash_h160].1;
            //     nonce = state.states[&senderPublicKey_hash_h160].0 + 1;
            //     let insert_balance = maxvalue;
            //     let insert_nonce = nonce;
            //     (*state).insert(senderPublicKey_hash_h160, insert_nonce, insert_balance);
            // }
            // else{
            //     maxvalue = 1000;
            //     nonce = 1;
            // }

            let value = rng.gen_range(0,cmp::min(10,currentBalance)) as u32;
            record.insert(senderIdx,(nonce, currentBalance-value));
            record.insert(recipientIdx, (recipientNonce,recipientBalance+value));
            
            info!("Generate one tx: {:} sends {:?} coins to {:}", 
                senderPublicKey_hash_h160,
                value,
                recipientPublicKey_hash_h160,
            );


            let transaction = Transaction{recipientAddr: recipientPublicKey.into(), value: value, accountNonce: nonce};
            let sig = sign(&transaction, &senderKeyPair);
            let signedT = SignedTransaction{signature: sig.as_ref().to_vec(), public_key: senderPublicKey.to_vec(), transaction: transaction};

            let temp_mempool = Arc::clone(&self.mempool);
            let mut mempool = temp_mempool.lock().unwrap();
            (*mempool).insert(&signedT);
            let mut broadcast_transactions_hashes = vec![];
            broadcast_transactions_hashes.push(signedT.clone().hash());
            self.server.broadcast(Message::NewTransactionHashes(broadcast_transactions_hashes));
            // println!("{:?}", record);

            
        }
    }
}
