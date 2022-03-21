// Adapted from simple_process_stats crate (https://github.com/robotty/simple-process-stats)

use procfs::process::Stat;
use std::fs::File;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Holds the retrieved basic statistics about the running process.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProcessStats {
    /// How much time this process has spent executing in user mode since it was started
    pub cpu_time_user: Duration,
    /// How much time this process has spent executing in kernel mode since it was started
    pub cpu_time_kernel: Duration,
    /// Size of the "resident" memory the process has allocated, in bytes.
    pub memory_usage_bytes: u64,
}

pub fn get_process_info(pid: u32) -> Result<ProcessStats, ()> {
    let bytes_per_page = procfs::page_size().map_err(|_| ())?;
    let ticks_per_second = procfs::ticks_per_second().map_err(|_| ())?;

    let path = PathBuf::from(format!("/proc/{}/stat", pid));
    let mut file_contents = Vec::new();
    File::open(path)
        .map_err(|_| ())?
        .read_to_end(&mut file_contents)
        .map_err(|_| ())?;

    let readable_string = Cursor::new(file_contents);
    let stat_file = Stat::from_reader(readable_string).map_err(|_e| ())?;

    let memory_usage_bytes = (stat_file.rss as u64) * (bytes_per_page as u64);
    let user_mode_seconds = (stat_file.utime as f64) / (ticks_per_second as f64);
    let kernel_mode_seconds = (stat_file.stime as f64) / (ticks_per_second as f64);

    Ok(ProcessStats {
        cpu_time_user: Duration::from_secs_f64(user_mode_seconds),
        cpu_time_kernel: Duration::from_secs_f64(kernel_mode_seconds),
        memory_usage_bytes,
    })
}
