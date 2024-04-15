use std::{os::unix::ffi::OsStrExt, path::PathBuf, str::FromStr, sync::{atomic::Ordering, Arc}, thread::{self, available_parallelism, JoinHandle}};
use std::collections::VecDeque;

use walkdir::WalkDir;

use crate::{arena::Arena, byte_storage::FilePath, WALKER_THREADS};


pub struct Walker {
    pub refs: Vec<Arc<parking_lot::RwLock<Arena<FilePath>>>>,
    handles: Vec<JoinHandle<Arena<u8>>>,
}

impl Walker {
    pub fn run(start: String) -> Self {
        let mut job_stack = VecDeque::new();
        let threads = available_parallelism().unwrap().get();
        WALKER_THREADS.store(threads as u32, Ordering::Relaxed);
        let mut handles = vec![];

        job_stack.push_back(PathBuf::from_str(&start).unwrap());

        let job_stack = Arc::new(parking_lot::Mutex::new(job_stack));
        let mut refs_storages = vec![];

        for thread_id in 0..threads {
            let job_stack = job_stack.clone();
            // let ref_storage = Arc::new(RefStorage::new());
            let ref_storage = Arc::new(parking_lot::RwLock::new(Arena::new()));
            refs_storages.push(ref_storage.clone());

            let handle = spawn(job_stack, ref_storage);
            handles.push(handle);
        }

        Self {
            refs: refs_storages,
            handles,
        }
    }

}

fn spawn(job_stack: Arc<parking_lot::Mutex<VecDeque<PathBuf>>>, ref_storage: Arc<parking_lot::RwLock<Arena<FilePath>>>) -> JoinHandle<Arena<u8>> {
    let handle = thread::spawn(move || {
        let mut arena = Arena::new();
        let mut ready_to_quit = false;
        loop {
            let mut stack = job_stack.lock();
            let dent = stack.pop_front();
            drop(stack);

            if let Some(dent) = dent {
                if ready_to_quit {
                    ready_to_quit = false;
                    WALKER_THREADS.fetch_add(1, Ordering::Relaxed);
                }
                let mut dir_items = vec![];
                let mut next_folders = vec![];

                for entry in WalkDir::new(dent).min_depth(1).max_depth(1).follow_root_links(false) {
                    let Ok(entry) = entry else { continue; };

                    let slice = arena.extend_and_get(entry.path().as_os_str().as_bytes());
                    dir_items.push(FilePath::new(slice));

                    if entry.file_type().is_dir() {
                    // if entry.path().is_dir() {
                        next_folders.push(entry.path().to_owned());
                    }
                }

                let mut stack = job_stack.lock();
                stack.extend(next_folders);
                drop(stack);
                if dir_items.len() > 0 {
                    ref_storage.write().extend(&dir_items);
                }
                //byte_storage.extend(dir_items);
            } else {
                if ready_to_quit && WALKER_THREADS.load(Ordering::Relaxed) == 0 {
                    break;
                }
                if !ready_to_quit {
                    ready_to_quit = true;
                    WALKER_THREADS.fetch_sub(1, Ordering::Relaxed);
                }
            }
        }

        arena
    });

    handle
}
