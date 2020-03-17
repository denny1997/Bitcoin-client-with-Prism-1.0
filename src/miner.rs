use crate::network::server::Handle as ServerHandle;

use log::info;

use crossbeam::channel::{unbounded, Receiver, Sender, TryRecvError};
use std::time;

use std::thread;

use std::sync::{Arc, Mutex};
use crate::blockchain::Blockchain;
use crate::transaction::{verify,Mempool,State,StatePerBlock};
use std::time::SystemTime;
use crate::crypto::merkle::MerkleTree;
use crate::crypto::hash::H256;
use crate::crypto::address::H160;
use rand::Rng;
use crate::transaction::SignedTransaction;
use crate::block::{Block,Header,Content};
use crate::crypto::hash::Hashable;
use crate::network::message::Message;
use serde::Serialize;

enum ControlSignal {
    Start(u64), // the number controls the lambda of interval between block generation
    Exit,
}

enum OperatingState {
    Paused,
    Run(u64),
    ShutDown,
}

pub struct Context {
    /// Channel for receiving control signal
    control_chan: Receiver<ControlSignal>,
    operating_state: OperatingState,
    server: ServerHandle,
    blockchain: Arc<Mutex<Blockchain>>,
    mempool: Arc<Mutex<Mempool>>,
    spb: Arc<Mutex<StatePerBlock>>,
}

#[derive(Clone)]
pub struct Handle {
    /// Channel for sending signal to the miner thread
    control_chan: Sender<ControlSignal>,
}

pub fn new(
    server: &ServerHandle, blockchain: &Arc<Mutex<Blockchain>>, mempool: &Arc<Mutex<Mempool>>, spb: &Arc<Mutex<StatePerBlock>>,
) -> (Context, Handle) {
    let (signal_chan_sender, signal_chan_receiver) = unbounded();

    let ctx = Context {
        control_chan: signal_chan_receiver,
        operating_state: OperatingState::Paused,
        server: server.clone(),
        blockchain: Arc::clone(blockchain),
        mempool: Arc::clone(mempool),
        spb: Arc::clone(spb),
    };

    let handle = Handle {
        control_chan: signal_chan_sender,
    };

    (ctx, handle)
}

impl Handle {
    pub fn exit(&self) {
        self.control_chan.send(ControlSignal::Exit).unwrap();
    }

    pub fn start(&self, lambda: u64) {
        self.control_chan
            .send(ControlSignal::Start(lambda))
            .unwrap();
    }

}

impl Context {
    pub fn start(mut self) {
        thread::Builder::new()
            .name("miner".to_string())
            .spawn(move || {
                self.miner_loop();
            })
            .unwrap();
        info!("Miner initialized into paused mode");
    }

    fn handle_control_signal(&mut self, signal: ControlSignal) {
        match signal {
            ControlSignal::Exit => {
                info!("Miner shutting down");
                self.operating_state = OperatingState::ShutDown;
            }
            ControlSignal::Start(i) => {
                info!("Miner starting in continuous mode with lambda {}", i);
                self.operating_state = OperatingState::Run(i);
            }
        }
    }

    fn miner_loop(&mut self) {
        // main mining loop

        let mut counter = 0;
        loop {
            // check and react to control signals
            match self.operating_state {
                OperatingState::Paused => {
                    let signal = self.control_chan.recv().unwrap();
                    self.handle_control_signal(signal);
                    continue;
                }
                OperatingState::ShutDown => {
                    return;
                }
                _ => match self.control_chan.try_recv() {
                    Ok(signal) => {
                        self.handle_control_signal(signal);
                    }
                    Err(TryRecvError::Empty) => {}
                    Err(TryRecvError::Disconnected) => panic!("Miner control channel detached"),
                },
            }
            if let OperatingState::ShutDown = self.operating_state {
                return;
            }
            
            // TODO: actual mining
            let temp = Arc::clone(&self.blockchain);
            //let mut blockchain = Arc::make_mut(&mut self.blockchain).lock().unwrap();
            let mut blockchain = temp.lock().unwrap();
            let temp_mempool = Arc::clone(&self.mempool);
            let mut mempool = temp_mempool.lock().unwrap();
            let temp_spb = Arc::clone(&self.spb);
            let mut spb = temp_spb.lock().unwrap();
            let parent = blockchain.tip();
            //println!("{:?}", parent);
            let timestamp:u128 = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_millis();
            let difficulty = blockchain.blocks[&parent].header.difficulty;

            let mut content:Vec<SignedTransaction> = vec![];
            let mut content_hash: Vec<H256> = vec![];
            let mempool_capacity = 16;
            if mempool.transactions.len() <= mempool_capacity {
                // println!("if");
                for (h, t) in mempool.transactions.iter() {
                    content.push((*t).clone());
                    content_hash.push((*h).clone());
                }
            } else {
                // println!("else");
                let mut num = 0;
                for (h, t) in mempool.clone().transactions.iter() {
                    content.push((*t).clone());
                    content_hash.push((*h).clone());
                    // (*mempool).transactions.remove(h);
                    num += 1;
                    if num == mempool_capacity {
                        break;
                    }
                }
            }
            // println!("{:?}!!!!!!!!!!!!!!!!!{:?}", content.len(), mempool.transactions.len());
            let root = MerkleTree::new(&content).root();
            // println!("!!!!!!!yes!!!!!");
            let mut rng = rand::thread_rng();
            let nonce: u32 = rng.gen();
            let header:Header = Header{parent:parent,nonce:nonce,difficulty:difficulty,timestamp:timestamp,merkle_root:root};
            let content:Content = Content{data:content};
            let block: Block = Block{header: header, content: content};
            // println!("Mempool length: {:?}", (*mempool).transactions.len());
            if block.hash()<= difficulty {
                //Arc::make_mut(&mut self.blockchain).get_mut().unwrap().insert(&block);
                let mut state = spb.spb[&block.header.parent].clone();
                let contents = &(&block.clone()).content.data;
                for signedTransaction in contents {
                    let signature = &signedTransaction.signature;
                    let public_key = &signedTransaction.public_key;
                    let transaction = &signedTransaction.transaction;

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
                }
                (*spb).insert(block.hash(),&state);
                (*blockchain).insert(&block);
                for key in content_hash.clone(){
                    (*mempool).transactions.remove(&key);
                }

                for (key, value) in (*mempool).transactions.clone().iter() {
                    for t in contents {
                        if (t.public_key == value.public_key) && (t.transaction.accountNonce == value.transaction.accountNonce) {
                            (*mempool).transactions.remove(&key);
                        }
                    }
                }
                let transactions_num = content_hash.len();
                // let currentTime = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_millis();
                // let durationSinceMined = currentTime - block.header.timestamp;
                let mut v = vec![];
                v.push(block.hash());
                self.server.broadcast(Message::NewBlockHashes(v));
                counter += 1;
                let encoded_block: Vec<u8> = bincode::serialize(&block).unwrap();
                println!("!!!!!!!!!!!!!!!I did it! Counter: {:?}, Block size is: {:?}, Block contains {:?} transactions", counter, encoded_block.len(), transactions_num);

                //self.blockchain = Arc::new(Mutex::new(blockchain));
            }

            if let OperatingState::Run(i) = self.operating_state {
                if i != 0 {
                    let interval = time::Duration::from_micros(i as u64);
                    thread::sleep(interval);
                }
            }
        }
    }
}
