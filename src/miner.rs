use crate::network::server::Handle as ServerHandle;

use log::info;

use crossbeam::channel::{unbounded, Receiver, Sender, TryRecvError};
use std::time;

use std::thread;

use std::sync::{Arc, Mutex};
use crate::blockchain::Blockchain;
use std::time::SystemTime;
use crate::crypto::merkle::MerkleTree;
use crate::crypto::hash::H256;
use rand::Rng;
use crate::transaction::Transaction;
use crate::block::{Block,Header,Content};
use crate::crypto::hash::Hashable;
use crate::network::message::Message;

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
}

#[derive(Clone)]
pub struct Handle {
    /// Channel for sending signal to the miner thread
    control_chan: Sender<ControlSignal>,
}

pub fn new(
    server: &ServerHandle, blockchain: &Arc<Mutex<Blockchain>>
) -> (Context, Handle) {
    let (signal_chan_sender, signal_chan_receiver) = unbounded();

    let ctx = Context {
        control_chan: signal_chan_receiver,
        operating_state: OperatingState::Paused,
        server: server.clone(),
        blockchain: Arc::clone(blockchain),
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
            let parent = blockchain.tip();
            //println!("{:?}", parent);
            let timestamp:u128 = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_millis();
            let difficulty = blockchain.blocks[&parent].header.difficulty;

            let content:Vec<Transaction> = vec![];
            let root = MerkleTree::new(&content).root();
            let mut rng = rand::thread_rng();
            let nonce: u32 = rng.gen();
            let header:Header = Header{parent:parent,nonce:nonce,difficulty:difficulty,timestamp:timestamp,merkle_root:root};
            let content:Content = Content{data:content};
            let block: Block = Block{header: header, content: content};

            if block.hash()<= difficulty {
                //Arc::make_mut(&mut self.blockchain).get_mut().unwrap().insert(&block);
                (*blockchain).insert(&block);
                let mut v = vec![];
                v.push(block.hash());
                self.server.broadcast(Message::NewBlockHashes(v));
                println!("{:?}", blockchain.blocks.len());
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
