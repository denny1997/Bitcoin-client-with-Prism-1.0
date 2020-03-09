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
use crate::transaction::{verify,sign,Mempool,Transaction,SignedTransaction};
use rand::Rng;
use crate::crypto::key_pair;
use std::time;



#[derive(Clone)]
pub struct Context {
    // msg_chan: channel::Receiver<(Vec<u8>, peer::Handle)>,
    num_worker: usize,
    server: ServerHandle,
    // blockchain: Arc<Mutex<Blockchain>>,
    // buffer: HashMap<H256, Block>,
    mempool: Arc<Mutex<Mempool>>,
    // keyPairs: Vec<Ed25519KeyPair>,
}

pub fn new(
    num_worker: usize,
    // msg_src: channel::Receiver<(Vec<u8>, peer::Handle)>,
    server: &ServerHandle,
    // blockchain: &Arc<Mutex<Blockchain>>,
    mempool: &Arc<Mutex<Mempool>>,
    // keyPairs: Vec<Ed25519KeyPair>,
) -> Context {
    Context {
        // msg_chan: msg_src,
        num_worker,
        server: server.clone(),
        // blockchain: Arc::clone(blockchain),
        // buffer: HashMap::new(),
        mempool: Arc::clone(mempool),
        // keyPairs: keyPairs.clone(),

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
            let duration = time::Duration::from_millis(3000);
            thread::sleep(duration);
            // println!("I'm running!");
            
            let mut rng = rand::thread_rng();
            let value:u32 = rng.gen();
            let key = key_pair::random();
            let public_key = key.public_key().as_ref();
            let public_key_hash = ring::digest::digest(&ring::digest::SHA256, public_key);
            let transaction = Transaction{recipientAddr: public_key_hash.as_ref().into(), value: value, accountNonce: 0};
            let sig = sign(&transaction, &key);
            let signedT = SignedTransaction{signature: sig.as_ref().to_vec(), public_key: public_key.to_vec(), transaction: transaction};

            let temp_mempool = Arc::clone(&self.mempool);
            let mut mempool = temp_mempool.lock().unwrap();
            (*mempool).insert(&signedT);
            let mut broadcast_transactions_hashes = vec![];
            broadcast_transactions_hashes.push(signedT.clone().hash());
            self.server.broadcast(Message::NewTransactionHashes(broadcast_transactions_hashes));

            
        }
    }
}
