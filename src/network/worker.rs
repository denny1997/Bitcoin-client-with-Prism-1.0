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

use std::thread;

#[derive(Clone)]
pub struct Context {
    msg_chan: channel::Receiver<(Vec<u8>, peer::Handle)>,
    num_worker: usize,
    server: ServerHandle,
    blockchain: Arc<Mutex<Blockchain>>,
    buffer: HashMap<H256, Block>,
}

pub fn new(
    num_worker: usize,
    msg_src: channel::Receiver<(Vec<u8>, peer::Handle)>,
    server: &ServerHandle,
    blockchain: &Arc<Mutex<Blockchain>>,
) -> Context {
    Context {
        msg_chan: msg_src,
        num_worker,
        server: server.clone(),
        blockchain: Arc::clone(blockchain),
        buffer: HashMap::new(),
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
            let temp = Arc::clone(&self.blockchain);
            let mut blockchain = temp.lock().unwrap();

            let msg = self.msg_chan.recv().unwrap();
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
                Message::NewBlockHashes(hashes) => {
                    debug!("NewBlockHashes");
                    let mut h = vec![];
                    for hash in hashes {
                        if !blockchain.blocks.contains_key(&hash) {
                            h.push(hash);
                        }
                    }
                    peer.write(Message::GetBlocks(h));
                }
                Message::GetBlocks(hashes) => {
                    debug!("GetBlocks");
                    let mut b = vec![];
                    for hash in hashes {
                        if blockchain.blocks.contains_key(&hash) {
                            b.push(blockchain.blocks[&hash].clone());
                        }
                    }
                    peer.write(Message::Blocks(b));
                }
                Message::Blocks(blocks) => {
                    debug!("Blocks");
                    let mut p = vec![];
                    for block in blocks {
                        if !blockchain.blocks.contains_key(&block.hash()) {
                            if !blockchain.blocks.contains_key(&block.header.parent){
                                self.buffer.insert(block.header.parent,block.clone());
                                p.push(block.header.parent)
                            } else {
                                (*blockchain).insert(&block);
                                let mut parent = block.hash();
                                let mut temp;
                                while self.buffer.contains_key(&parent) {
                                    (*blockchain).insert(&self.buffer[&parent]);                          
                                    temp = self.buffer[&parent].hash();
                                    self.buffer.remove(&parent);
                                    parent = temp;
                                }
                            }                           
                        }
                    }
                    if p.len() > 0 {
                        peer.write(Message::GetBlocks(p));
                    }
                }
            }
        }
    }
}
