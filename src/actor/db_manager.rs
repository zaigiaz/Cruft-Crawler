use steady_state::*;
use std::error::Error;
use crate::actor::crawler::FileMeta;
use sled::Db;

const BATCH_SIZE: usize = 1;

pub async fn run(actor: SteadyActorShadow, 
                 crawler_rx: SteadyRx<FileMeta> ) -> Result<(),Box<dyn Error>> {

    internal_behavior(actor.into_spotlight([&crawler_rx], []), crawler_rx).await
}


async fn internal_behavior<A: SteadyActor>(mut actor: A,
                                           crawler_rx: SteadyRx<FileMeta>) -> Result<(),Box<dyn Error>> {

    let mut crawler_rx = crawler_rx.lock().await;


    // TODO: example code that I need to change
    let db: sled::Db = sled::open("../db").unwrap();
    let _ = db.insert(b"yo!", b"v1");


    while actor.is_running(|| crawler_rx.is_closed_and_empty()) {

	actor.wait_avail(&mut crawler_rx, BATCH_SIZE).await;
	let recieved = actor.try_take(&mut crawler_rx);

	recieved.expect("expected FileMeta Struct (db_actor)").meta_print();
	}

  Ok(())
}


// add db entry given key and value pair
fn db_add(key: i32, value: FileMeta, db: sled::Db) -> Result<(), Box<dyn Error>> {

    println!("{}", key);
    value.meta_print();


    // value.serialize();

    db.insert(b"yo", b"no");

Ok(())
}


// edit db entry given key
fn db_edit(key: i32) -> Result<(), Box<dyn Error>> {

// get key from db and edit after deserializing

Ok(())
}


// remove db entry given key
fn db_remove(key: i32) -> Result<(), Box<dyn Error>> {

// remove entry based on key

Ok(())
}


