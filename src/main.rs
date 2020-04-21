#[cfg(test)]
#[macro_use]
extern crate hex_literal;

pub mod api;
pub mod block;
pub mod blockchain;
pub mod crypto;
pub mod miner;
pub mod network;
pub mod transaction;

use clap::clap_app;
use crossbeam::channel;
use log::{error, info};
use api::Server as ApiServer;
use network::{server, worker, generator};
use std::net;
use std::process;
use std::thread;
use std::time;

use std::sync::{Arc, Mutex};
use crate::blockchain::Blockchain;
use crate::transaction::{Mempool, TxBlockMempool, State, StatePerBlock};
use std::collections::HashMap;
use crate::crypto::hash::{Hashable,H256};
use crate::block::Block;
use crate::crypto::key_pair;
use ring::signature::{Ed25519KeyPair};


fn main() {
    // parse command line arguments
    let matches = clap_app!(Bitcoin =>
     (version: "0.1")
     (about: "Bitcoin client")
     (@arg verbose: -v ... "Increases the verbosity of logging")
     (@arg peer_addr: --p2p [ADDR] default_value("127.0.0.1:6000") "Sets the IP address and the port of the P2P server")
     (@arg api_addr: --api [ADDR] default_value("127.0.0.1:7000") "Sets the IP address and the port of the API server")
     (@arg known_peer: -c --connect ... [PEER] "Sets the peers to connect to at start")
     (@arg p2p_workers: --("p2p-workers") [INT] default_value("4") "Sets the number of worker threads for P2P server")
     (@arg generate: -g --("generator") [INT] default_value("0") "Sets generator status")
     (@arg attack: -a --("attacker") [INT] default_value("0") "Sets attacker status, 0: no attack, 1: spamming attack, 2: cencorship attack, 3: both attacks")
    )
    .get_matches();

    // init logger
    let verbosity = matches.occurrences_of("verbose") as usize;
    stderrlog::new().verbosity(verbosity).init().unwrap();

    // parse p2p server address
    let p2p_addr = matches
        .value_of("peer_addr")
        .unwrap()
        .parse::<net::SocketAddr>()
        .unwrap_or_else(|e| {
            error!("Error parsing P2P server address: {}", e);
            process::exit(1);
        });

    // parse api server address
    let api_addr = matches
        .value_of("api_addr")
        .unwrap()
        .parse::<net::SocketAddr>()
        .unwrap_or_else(|e| {
            error!("Error parsing API server address: {}", e);
            process::exit(1);
        });

    // create channels between server and worker
    let (msg_tx, msg_rx) = channel::unbounded();

    let temp_blockchain = Blockchain::new();
    let mut blockchain = Arc::new(Mutex::new(temp_blockchain.clone()));
    let mut mempool = Arc::new(Mutex::new(Mempool::new()));
    let mut txBlockmempool = Arc::new(Mutex::new(TxBlockMempool::new()));
    let mut txBlockOrderedList = Arc::new(Mutex::new(Vec::new()));
    // let mut keys = vec![];
    let mut key_hashtable: HashMap<u32, Ed25519KeyPair>=HashMap::new();
    for i in 0..5 {
        let key = key_pair::random();
        key_hashtable.insert(i, key);
        // keys.push(key);
    }

    let temp_state = State::new();
    let mut state = Arc::new(Mutex::new(temp_state.clone()));
    let mut spb = Arc::new(Mutex::new(StatePerBlock::new(temp_blockchain.genesis, temp_state)));
    let mut key_set = Arc::new(Mutex::new(key_hashtable));

    // start the p2p server
    let (server_ctx, server) = server::new(p2p_addr, msg_tx).unwrap();
    server_ctx.start().unwrap();

    // start the worker
    let p2p_workers = matches
        .value_of("p2p_workers")
        .unwrap()
        .parse::<usize>()
        .unwrap_or_else(|e| {
            error!("Error parsing P2P workers: {}", e);
            process::exit(1);
        });


    let mut new_Hashmap = Arc::new(Mutex::new(HashMap::new()));
    let worker_ctx = worker::new(
        p2p_workers,
        msg_rx,
        &server,
        &blockchain,
        &new_Hashmap,
        &mempool,
        &txBlockmempool, 
        &txBlockOrderedList,
        // &state,
        &spb,
    );
    worker_ctx.start();

    let attack = matches.value_of("attack").unwrap().parse::<usize>().unwrap();
    if Some("1") == matches.value_of("generate") {
        
        // start the transaction generator
        let generator = generator::new(
            1,
            &server,
            &mempool,
            &state,
            &key_set,
            attack,
        );
        generator.start();
    }


    // start the miner
    let (miner_ctx, miner) = miner::new(
        &server, &blockchain, &mempool, &txBlockmempool, &txBlockOrderedList, &spb, attack,
    );
    miner_ctx.start();

    // connect to known peers
    if let Some(known_peers) = matches.values_of("known_peer") {
        let known_peers: Vec<String> = known_peers.map(|x| x.to_owned()).collect();
        let server = server.clone();
        thread::spawn(move || {
            for peer in known_peers {
                loop {
                    let addr = match peer.parse::<net::SocketAddr>() {
                        Ok(x) => x,
                        Err(e) => {
                            error!("Error parsing peer address {}: {}", &peer, e);
                            break;
                        }
                    };
                    match server.connect(addr) {
                        Ok(_) => {
                            info!("Connected to outgoing peer {}", &addr);
                            break;
                        }
                        Err(e) => {
                            error!(
                                "Error connecting to peer {}, retrying in one second: {}",
                                addr, e
                            );
                            thread::sleep(time::Duration::from_millis(1000));
                            continue;
                        }
                    }
                }
            }
        });
    }


    // start the API server
    ApiServer::start(
        api_addr,
        &miner,
        &server,
    );

    loop {
        std::thread::park();
    }
}
