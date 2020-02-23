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
            let msg = self.msg_chan.recv().unwrap();
            // println!("3");
            let temp = Arc::clone(&self.blockchain);
            // println!("1");
            let mut blockchain = temp.lock().unwrap();
            // println!("2");
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
                        if !blockchain.blocks.contains_key(&block.hash()) {
                            if !blockchain.blocks.contains_key(&block.header.parent){
                                self.buffer.insert(block.header.parent,block.clone());
                                debug!("Parent not recieved yet");
                                p.push(block.header.parent)
                            } else {
                                (*blockchain).insert(&block);
                                broadcast_blocks_hashes.push(block.clone().hash());
                                let mut parent = block.hash();
                                let mut temp;
                                while self.buffer.contains_key(&parent) {
                                    println!("stucked! TAT");
                                    (*blockchain).insert(&self.buffer[&parent]); 
                                    broadcast_blocks_hashes.push((self.buffer[&parent]).clone().hash());                         
                                    temp = self.buffer[&parent].hash();
                                    self.buffer.remove(&parent);
                                    parent = temp;
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
