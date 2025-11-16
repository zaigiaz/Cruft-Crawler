use steady_state::*;
use std::path::{Path, PathBuf};
use std::error::Error;
use filetime::FileTime;

#[allow(unused_imports)]
use std::time::Duration;

// have here for implementing state later
// use crate::db_manager::db_state;

// derived fn that allow cloning and printing
#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub(crate) struct FileMeta {
    pub rel_path: PathBuf,
    pub abs_path: PathBuf,
    pub file_name: String,
    pub is_file: bool,
    pub size: u64,
    pub modified: Option<filetime::FileTime>,
    pub created: Option<filetime::FileTime>,
    pub readonly: bool,
} 

// for easy debugging if needed 
impl FileMeta {
   pub fn meta_print(&self) {
	println!("Printing Metadata Object -----------");
	println!("Absolute_Path: {:?}", self.abs_path);
	println!("Relative_Path: {:?}", self.rel_path);
	println!("File_Name: {}",       self.file_name);
	println!("is_file: {}",         self.is_file);
	println!("size: {}",            self.size);
	println!("modified: {:?}",      self.modified.unwrap().seconds() / 60);
	println!("created: {:?}",       self.created.unwrap().seconds() / 60);
	println!("read-only: {}",       self.readonly);
	println!("Printing Metadata Object -----------\n");
    }
}


//TODO: implement Walkdir to recursively get different directories
//TODO: see about replacing SystemTime, with another field for better parsing
//TODO: import hashing crate and hash first chunk of files or vector embedding
//TODO: hard-code values for different file-types and how to treat them
//TODO: replace SahomeDB back with Sled
//TODO: create DB schema for Sled / SahomeDB
//TODO: Implement state or communication to Database to ensure its crawling in correct location on actor failure

// run function 
pub async fn run(actor: SteadyActorShadow,
                 crawler_tx: SteadyTx<FileMeta>) -> Result<(),Box<dyn Error>> {

    internal_behavior(actor.into_spotlight([], [&crawler_tx]), crawler_tx).await
}


// Internal behaviour for the actor
async fn internal_behavior<A: SteadyActor>(mut actor: A,
					   crawler_tx: SteadyTx<FileMeta> ) -> Result<(),Box<dyn Error>> {

    let mut crawler_tx = crawler_tx.lock().await;

    let dir = Path::new("crawl_test/");
    let metas = visit_dir(dir)?;

    while actor.is_running(|| crawler_tx.mark_closed()) {

	// wait before channel is vacant before sending
	// note that depending on the situation you can call the await_for_***() function for different scenarios
	for m in &metas {
	actor.wait_vacant(&mut crawler_tx, 1).await; 

	// awaiting either sleeping the thread or actor.wait_periodic() cause's a return None issue at the end
	// actor.wait_periodic(Duration::from_millis(1000)).await;

	actor.try_send(&mut crawler_tx, m.clone()).expect("couldn't send to DB");
	}

	actor.request_shutdown().await
    }
	return Ok(());
}



// function to visit test directory and return metadata of each file and insert into metadata struct
// then send to the db_manager actor (although this doesnt occur in this function)
pub fn visit_dir(dir: &Path) -> Result<Vec<FileMeta>, Box<dyn Error>> {
    let mut metas = Vec::new();

    // Read the directory (non-recursive)
    for entry_res in std::fs::read_dir(dir)? {
        let entry = entry_res?;
        let rel_path = entry.path();
	 let abs_path = std::path::absolute(&rel_path)?;
        let file_name = entry
            .file_name()
            .into_string()
            .unwrap_or_else(|os| os.to_string_lossy().into_owned());
	
        // Try to get metadata; if failing for a specific entry, skip it but continue
        match entry.metadata() {
            Ok(md) => {
                let is_file = md.is_file();
                let size = md.len();
                let modified = FileTime::from_last_modification_time(&md);
                let created = FileTime::from_creation_time(&md);
                let readonly = md.permissions().readonly();

                metas.push(FileMeta {
                    rel_path,
		      abs_path,
                    file_name,
                    is_file,
                    size,
                    modified: Some(modified),
                    created,
                    readonly,
                });
            }
            Err(e) => {
                eprintln!("warning: cannot stat {}: {}", file_name, e);
            }
        }
    }
    Ok(metas)
}
