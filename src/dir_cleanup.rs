use parking_lot::lock_api::RawMutex;
use parking_lot::Mutex;
use std::fs::{create_dir_all, remove_dir_all};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI32, Ordering};

// pgid of the tool currently being supervised; 0 means none. Used so that
// Ctrl+C / panic handlers can kill the whole child process tree even though
// each child is placed in its own process group (so the terminal's SIGINT
// does not reach it on its own).
static CURRENT_CHILD_PGID: AtomicI32 = AtomicI32::new(0);

pub fn set_current_child_pgid(pgid: i32) {
    CURRENT_CHILD_PGID.store(pgid, Ordering::SeqCst);
}

pub fn clear_current_child_pgid(pgid: i32) {
    let _ = CURRENT_CHILD_PGID.compare_exchange(pgid, 0, Ordering::SeqCst, Ordering::SeqCst);
}

pub fn kill_current_child() {
    let pgid = CURRENT_CHILD_PGID.swap(0, Ordering::SeqCst);
    if pgid > 0 {
        eprintln!("Killing running tool process group {}", pgid);
        unsafe {
            libc::killpg(pgid, libc::SIGKILL);
        }
    }
}

pub struct DirGuard {
    path: PathBuf,
}

impl AsRef<Path> for DirGuard {
    fn as_ref(&self) -> &Path {
        &self.path
    }
}

impl Drop for DirGuard {
    fn drop(&mut self) {
        if remove_dir_all(&self.path).is_err() {
            println!("WARNING: Cannot remove the working directory!");
        }
        TEMP_DIRS.lock().retain(|path| path != &self.path);
    }
}

static TEMP_DIRS: Mutex<Vec<PathBuf>> = Mutex::const_new(RawMutex::INIT, Vec::new());

pub fn remove_dirs_on_panic() {
    kill_current_child();
    println!("Removing temp directories!");
    let mut tmp_dirs = TEMP_DIRS.lock();
    for dir in tmp_dirs.iter() {
        println!("Removing dir {}...", dir.display());
        remove_dir_all(dir);
    }
    tmp_dirs.clear();
}

pub fn create_dir_with_guard(dir: impl AsRef<Path>) -> Option<DirGuard> {
    if dir.as_ref().exists() && dir.as_ref().read_dir().unwrap().next().is_some() {
        println!(
            "Directory '{}' already exists, aborting!",
            dir.as_ref().display()
        );
        return None;
    }

    create_dir_all(&dir).ok().map(|_| {
        let path = dir.as_ref().to_path_buf();
        TEMP_DIRS.lock().push(path.clone());
        DirGuard { path }
    })
}
