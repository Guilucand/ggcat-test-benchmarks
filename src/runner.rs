use crate::config::{Dataset, Tool};
use fork::Fork;
use rlimit::Resource;

use crate::stats::get_process_info;
use cgroups_rs::cgroup_builder::*;
use cgroups_rs::*;
use serde::{Deserialize, Serialize};
use std::cmp::min;
use std::collections::{HashMap, VecDeque};
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Write};
use std::mem::MaybeUninit;
use std::os::raw::c_int;
use std::os::unix::raw::pid_t;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::Thread;
use std::time::{Duration, Instant};
use std::{env, io};
use walkdir::WalkDir;

pub struct Runner {}

pub struct Parameters {
    pub max_threads: usize,
    pub k: usize,
    pub multiplicity: usize,
    pub output_file: String,
    pub canonical_file: String,
    pub temp_dir: String,
    pub log_file: PathBuf,
    pub memory_gb: Option<f64>,
    pub size_check_time: Duration,
    pub query_files: (Option<String>, Option<String>),
    pub timeout: Option<Duration>,
    pub enforce_threads: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RunResults {
    pub command_line: String,
    pub max_memory_gb: f64,
    pub max_measured_memory_gb: f64,
    pub user_time_secs: f64,
    pub system_time_secs: f64,
    pub real_time_secs: f64,
    pub total_written_gb: f64,
    pub total_read_gb: f64,
    pub max_used_disk_gb: f64,
    pub output_file_sizes: Vec<(String, (u64, f64))>,
    pub has_completed: bool,
    #[serde(default)]
    pub timed_out: bool,
    #[serde(default)]
    pub timeout_secs: Option<f64>,
    #[serde(default)]
    pub deadlock_detected: bool,
}

fn absolute_path(path: impl AsRef<Path>) -> io::Result<PathBuf> {
    let path = path.as_ref();

    let absolute_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        env::current_dir()?.join(path)
    };

    Ok(absolute_path)
}

fn get_dir_size(path: impl AsRef<Path>) -> u64 {
    let mut dir_size = 0;
    for entry in WalkDir::new(path) {
        if let Ok(file) = entry {
            dir_size += file.metadata().map(|m| m.len()).unwrap_or(0)
        }
    }
    dir_size
}

/// Sum cumulative CPU time (user + kernel) across every process whose pgrp
/// equals `pgid`. Returns None if /proc cannot be enumerated at all.
fn collect_pgid_cpu_time(pgid: i32) -> Option<Duration> {
    let ticks_per_second = procfs::ticks_per_second().ok()?;
    let mut total_ticks: u64 = 0;
    for entry in std::fs::read_dir("/proc").ok()?.filter_map(|e| e.ok()) {
        let name = entry.file_name();
        let Some(name_str) = name.to_str() else { continue };
        if name_str.parse::<u32>().is_err() {
            continue;
        }
        let stat_path = entry.path().join("stat");
        let mut file = match File::open(&stat_path) {
            Ok(f) => f,
            Err(_) => continue,
        };
        let mut buf = Vec::new();
        if file.read_to_end(&mut buf).is_err() {
            continue;
        }
        let stat = match procfs::process::Stat::from_reader(std::io::Cursor::new(buf)) {
            Ok(s) => s,
            Err(_) => continue,
        };
        if stat.pgrp == pgid {
            total_ticks = total_ticks
                .saturating_add(stat.utime)
                .saturating_add(stat.stime);
        }
    }
    Some(Duration::from_secs_f64(
        total_ticks as f64 / ticks_per_second as f64,
    ))
}

impl Runner {
    pub fn run_tool(
        base_dir: impl AsRef<Path>,
        tool: Tool,
        dataset_name: String,
        input_files: &Vec<PathBuf>,
        parameters: Parameters,
    ) -> RunResults {
        let input_files_string = input_files
            .iter()
            .map(|f| f.as_os_str().to_str().unwrap().to_string())
            .collect::<Vec<String>>();

        // Acquire a handle for the cgroup hierarchy.
        #[cfg(feature = "cpu-limit")]
        let cg: Cgroup = {
            let hier = cgroups_rs::hierarchies::auto();

            // Use the builder pattern (see the documentation to create the control group)
            //
            // This creates a control group named "example" in the V1 hierarchy.

            let max_cores = min(num_cpus::get(), parameters.max_threads);

            CgroupBuilder::new("genome-benchmark-cgroup")
                .cpu()
                .period(100000)
                .quota(100000 * max_cores as i64)
                .cpus(format!("{}-{}", 0, max_cores - 1))
                .done()
                .build(hier)
        };

        let input_files_list_file_name =
            std::env::temp_dir().join(format!("input-files-{}.txt", dataset_name));
        {
            let mut input_files_list = File::create(&input_files_list_file_name).unwrap();
            input_files_list.write_all(input_files_string.join("\n").as_bytes());
            input_files_list.write_all(b"\n");
        }

        let program_arguments: HashMap<&str, Vec<String>> = [
            ("<THREADS>", vec![parameters.max_threads.to_string()]),
            ("<KVALUE>", vec![parameters.k.to_string()]),
            ("<MULTIPLICITY>", vec![parameters.multiplicity.to_string()]),
            ("<INPUT_FILES>", input_files_string.clone()),
            ("<INPUT_FILES_LIST>", {
                let mut vec = vec![];

                if tool.use_prefix_for_list.unwrap_or(false) {
                    if parameters.multiplicity > 1 {
                        if let Some(reads_prefix) = tool.reads_arg_prefix.clone() {
                            vec.push(reads_prefix)
                        }
                    } else {
                        if let Some(sequences_prefix) = tool.sequences_arg_prefix.clone() {
                            vec.push(sequences_prefix)
                        }
                    }
                }

                vec.push(input_files_list_file_name.to_str().unwrap().to_string());
                vec
            }),
            ("<INPUT_FILES_READS>", {
                if let Some(reads_prefix) = tool.reads_arg_prefix {
                    if parameters.multiplicity > 1 {
                        input_files_string
                            .iter()
                            .map(|x| vec![reads_prefix.clone(), x.clone()])
                            .flatten()
                            .collect()
                    } else {
                        vec![]
                    }
                } else {
                    vec![]
                }
            }),
            ("<INPUT_FILES_SEQUENCES>", {
                if let Some(sequences_prefix) = tool.sequences_arg_prefix {
                    if parameters.multiplicity == 1 {
                        input_files_string
                            .iter()
                            .map(|x| vec![sequences_prefix.clone(), x.clone()])
                            .flatten()
                            .collect()
                    } else {
                        vec![]
                    }
                } else {
                    vec![]
                }
            }),
            (
                "<OUTPUT_FILE>",
                vec![absolute_path(&parameters.output_file)
                    .unwrap()
                    .into_os_string()
                    .into_string()
                    .unwrap()],
            ),
            (
                "<TEMP_DIR>",
                vec![absolute_path(&parameters.temp_dir)
                    .unwrap()
                    .into_os_string()
                    .into_string()
                    .unwrap()],
            ),
            (
                "<MAX_MEMORY>",
                vec![format!("{:.2}", parameters.memory_gb.unwrap_or(0.0))],
            ),
            ("<INPUT_FILES>", input_files_string.clone()),
            ("<INPUT_GRAPH>", input_files_string.clone()),
            (
                "<INPUT_QUERY>",
                vec![parameters.query_files.0.clone().unwrap_or(String::new())],
            ),
            (
                "<INPUT_COLORS>",
                vec![parameters.query_files.1.clone().unwrap_or(String::new())],
            ),
        ]
        .iter()
        .cloned()
        .collect();

        let tool_path = if tool.path.is_absolute() {
            tool.path
        } else {
            base_dir.as_ref().join(tool.path).to_path_buf()
        };

        let mut arguments = tool.arguments.split(" ").collect::<Vec<_>>();

        let mut i = 0;
        while i < arguments.len() {
            if program_arguments.contains_key(arguments[i]) {
                let args = &program_arguments[arguments[i]];
                arguments.remove(i);
                for (j, arg) in args.iter().enumerate() {
                    arguments.insert(i + j, arg);
                }
            } else {
                i += 1;
            }
        }

        let start_time = Instant::now();

        println!(
            "Running tool {} with dataset {} K = {} threads = {}",
            &tool.name, &dataset_name, parameters.k, parameters.max_threads
        );
        eprintln!("{} {}", tool_path.display(), arguments.join(" "));

        // Reset the max_rss for the current process
        {
            File::options()
                .write(true)
                .open("/proc/self/clear_refs")
                .map(|mut f| {
                    f.write(b"5");
                    f.flush();
                })
                .unwrap_or(());
        }

        let mut command_builder = std::process::Command::new(&tool_path);
        command_builder
            .args(arguments.as_slice())
            .stdout(File::create(&parameters.log_file).unwrap())
            .stderr(File::create(parameters.log_file.with_extension("stderr")).unwrap());

        // Compute how many CPUs we will pin the tool to. We cap parallelism
        // by setting CPU affinity in pre_exec, which is inherited by every
        // thread/child the tool spawns and is enforced by the kernel —
        // unlike cpulimit, which is unreliable against many-threaded tools.
        let total_cpus = num_cpus::get();
        let affinity_cpus = if parameters.enforce_threads {
            Some(parameters.max_threads.min(total_cpus).max(1))
        } else {
            None
        };
        if let Some(n) = affinity_cpus {
            println!(
                "Pinning tool to CPUs 0..{} via sched_setaffinity (max_threads={}, host has {})",
                n,
                parameters.max_threads,
                total_cpus
            );
        }

        // pre_exec: put the child in its own process group (so killpg can
        // reap the whole tree on timeout / Ctrl+C / panic) and optionally
        // restrict its CPU affinity to enforce the thread budget.
        unsafe {
            use std::os::unix::process::CommandExt;
            command_builder.pre_exec(move || {
                if libc::setpgid(0, 0) != 0 {
                    return Err(io::Error::last_os_error());
                }
                if let Some(n) = affinity_cpus {
                    let mut cpuset: libc::cpu_set_t = std::mem::zeroed();
                    libc::CPU_ZERO(&mut cpuset);
                    for i in 0..n {
                        libc::CPU_SET(i, &mut cpuset);
                    }
                    if libc::sched_setaffinity(
                        0,
                        std::mem::size_of::<libc::cpu_set_t>(),
                        &cpuset,
                    ) != 0
                    {
                        return Err(io::Error::last_os_error());
                    }
                }
                Ok(())
            });
        }

        let mut command = command_builder.spawn().unwrap();

        // The pre_exec hook put the child in its own process group, so its
        // pgid equals its pid. Register it so Ctrl+C / panic handlers can
        // kill the whole tree, since the terminal SIGINT no longer reaches it.
        let child_pgid = command.id() as i32;
        crate::dir_cleanup::set_current_child_pgid(child_pgid);

        let is_finished = Arc::new(AtomicBool::new(false));
        let timed_out_flag = Arc::new(AtomicBool::new(false));
        let deadlock_detected_flag = Arc::new(AtomicBool::new(false));

        let pid = command.id();

        let is_finished_thr = is_finished.clone();
        let temp_dir_thr = parameters.temp_dir.clone();
        let out_dir_thr = PathBuf::from(&parameters.output_file)
            .parent()
            .unwrap()
            .to_path_buf();
        let out_dir_for_final_size = out_dir_thr.clone();

        let maximum_disk_usage = Arc::new(AtomicU64::new(0));
        let maximum_rss_usage = Arc::new(AtomicU64::new(0));

        let maximum_disk_usage_thr = maximum_disk_usage.clone();
        let maximum_rss_usage_thr = maximum_rss_usage.clone();

        let timeout_thread = parameters.timeout.map(|timeout| {
            let is_finished_thr = is_finished.clone();
            let timed_out_thr = timed_out_flag.clone();
            let pgid = pid as i32;
            std::thread::spawn(move || {
                let started = Instant::now();
                let poll = Duration::from_millis(500);
                while !is_finished_thr.load(Ordering::Relaxed) {
                    if started.elapsed() >= timeout {
                        timed_out_thr.store(true, Ordering::Relaxed);
                        eprintln!(
                            "Tool exceeded timeout of {:.1}s, killing process group {}",
                            timeout.as_secs_f64(),
                            pgid
                        );
                        unsafe {
                            libc::killpg(pgid, libc::SIGKILL);
                        }
                        return;
                    }
                    std::thread::sleep(poll);
                }
            })
        });

        // Deadlock detection: kill the tool if its process tree averages less
        // than 0.3 cores over the window below. Window = min(1h, timeout/2),
        // falling back to 1h when no timeout is configured.
        let deadlock_window: Duration = {
            let half_timeout = parameters
                .timeout
                .map(|t| t / 2)
                .unwrap_or(Duration::from_secs(3600));
            Duration::from_secs(3600).min(half_timeout)
        };
        const DEADLOCK_CHECK_INTERVAL: Duration = Duration::from_secs(300);
        const DEADLOCK_SUMMARY_INTERVAL: Duration = Duration::from_secs(7200);
        const DEADLOCK_THRESHOLD_CORES: f64 = 0.3;

        println!(
            "Deadlock monitor armed for pgid {}: window {:.0}s, threshold {:.2} cores, check every {:.0}s",
            child_pgid,
            deadlock_window.as_secs_f64(),
            DEADLOCK_THRESHOLD_CORES,
            DEADLOCK_CHECK_INTERVAL.as_secs_f64(),
        );

        let deadlock_thread = {
            let is_finished_thr = is_finished.clone();
            let deadlock_flag_thr = deadlock_detected_flag.clone();
            let pgid = child_pgid;
            let window = deadlock_window;
            std::thread::spawn(move || {
                let start = Instant::now();
                // (timestamp, cumulative cpu time of the process tree)
                let mut history: VecDeque<(Instant, Duration)> = VecDeque::new();
                if let Some(cpu) = collect_pgid_cpu_time(pgid) {
                    history.push_back((start, cpu));
                }
                let mut last_summary = start;
                let mut last_check = start;
                let poll_step = Duration::from_secs(5);
                loop {
                    std::thread::sleep(poll_step);
                    if is_finished_thr.load(Ordering::Relaxed) {
                        return;
                    }
                    let now = Instant::now();

                    if now.duration_since(last_summary) >= DEADLOCK_SUMMARY_INTERVAL {
                        last_summary = now;
                        if let Some(current_cpu) = collect_pgid_cpu_time(pgid) {
                            let elapsed = now.duration_since(start).as_secs_f64().max(1e-3);
                            let avg_cores = current_cpu.as_secs_f64() / elapsed;
                            println!(
                                "[deadlock-monitor] pgid {}: average CPU {:.2} cores over {:.0}s",
                                pgid, avg_cores, elapsed
                            );
                        }
                    }

                    if now.duration_since(last_check) < DEADLOCK_CHECK_INTERVAL {
                        continue;
                    }
                    last_check = now;
                    let Some(current_cpu) = collect_pgid_cpu_time(pgid) else {
                        continue;
                    };
                    history.push_back((now, current_cpu));

                    // Keep the oldest sample that is still >= window old.
                    // Drop earlier samples once the next-oldest also covers the window.
                    while history.len() >= 2 {
                        if now.duration_since(history[1].0) >= window {
                            history.pop_front();
                        } else {
                            break;
                        }
                    }

                    let anchor = *history.front().unwrap();
                    let anchor_age = now.duration_since(anchor.0);
                    if anchor_age >= window {
                        let dcpu = current_cpu
                            .as_secs_f64()
                            - anchor.1.as_secs_f64();
                        let dt = anchor_age.as_secs_f64();
                        let avg_cores = if dt > 0.0 { dcpu / dt } else { 0.0 };
                        if avg_cores < DEADLOCK_THRESHOLD_CORES {
                            eprintln!(
                                "[deadlock-monitor] DEADLOCK detected for pgid {}: {:.3} cores avg over {:.0}s (threshold {:.2}), killing process group",
                                pgid, avg_cores, dt, DEADLOCK_THRESHOLD_CORES
                            );
                            deadlock_flag_thr.store(true, Ordering::Relaxed);
                            unsafe {
                                libc::killpg(pgid, libc::SIGKILL);
                            }
                            return;
                        }
                    }
                }
            })
        };

        let maximum_disk_usage_thread = std::thread::spawn(move || {
            while !is_finished_thr.load(Ordering::Relaxed) {
                maximum_disk_usage_thr.fetch_max(
                    get_dir_size(&temp_dir_thr) + get_dir_size(&out_dir_thr),
                    Ordering::Relaxed,
                );
                maximum_rss_usage_thr.fetch_max(
                    get_process_info(pid)
                        .map(|x| x.memory_usage_bytes)
                        .unwrap_or(0),
                    Ordering::Relaxed,
                );
                std::thread::sleep(parameters.size_check_time);
            }
        });

        #[cfg(feature = "cpu-limit")]
        cg.add_task(CgroupPid::from(&command)).expect(
            "Cannot set correct cgroup, please initialize as root with the start subcommand",
        );

        let mut rusage: libc::rusage;
        unsafe {
            let mut status = 0;
            rusage = MaybeUninit::zeroed().assume_init();
            libc::wait4(
                command.id() as pid_t,
                &mut status as *mut c_int,
                0,
                &mut rusage as *mut libc::rusage,
            );
        }
        let total_seconds = start_time.elapsed().as_secs_f64();

        is_finished.store(true, Ordering::Relaxed);
        crate::dir_cleanup::clear_current_child_pgid(child_pgid);
        maximum_disk_usage_thread.join();
        if let Some(t) = timeout_thread {
            let _ = t.join();
        }
        let _ = deadlock_thread.join();
        let timed_out = timed_out_flag.load(Ordering::Relaxed);
        let deadlock_detected = deadlock_detected_flag.load(Ordering::Relaxed);

        let mut has_completed = false;

        let output_result = {
            let output_file = Path::new(&parameters.output_file);
            let output_parent = output_file.parent().unwrap();
            let mut result = None;
            for file in output_parent.read_dir().unwrap() {
                let entry = file.unwrap();
                let file_name = entry.file_name().to_str().unwrap().to_string();

                if file_name.starts_with(output_file.file_name().unwrap().to_str().unwrap())
                    && file_name.ends_with(".gfa")
                {
                    // Mark gfa files as completed but do not process them
                    has_completed = true;
                }

                if file_name.starts_with(output_file.file_name().unwrap().to_str().unwrap())
                    && file_name.ends_with(".fa")
                {
                    result = Some(entry.path());
                    break;
                }
            }
            result
        };

        if let Some(result) = output_result {
            if parameters.query_files.0.is_none() {
                canonical_kmers::canonicalize(
                    &result,
                    parameters.canonical_file,
                    parameters.k,
                    false,
                );
            }
            has_completed = true;
        }

        RunResults {
            command_line: format!("{} {}", tool_path.display(), arguments.join(" ")),
            max_memory_gb: rusage.ru_maxrss as f64 / (1024.0 * 1024.0),
            max_measured_memory_gb: maximum_rss_usage.load(Ordering::Relaxed) as f64
                / (1024.0 * 1024.0),
            user_time_secs: rusage.ru_utime.tv_sec as f64
                + (rusage.ru_utime.tv_usec as f64 / 1000000.0),
            system_time_secs: rusage.ru_stime.tv_sec as f64
                + (rusage.ru_stime.tv_usec as f64 / 1000000.0),
            real_time_secs: total_seconds,
            total_written_gb: rusage.ru_oublock as f64 / 2048.0 / 1024.0,
            total_read_gb: rusage.ru_inblock as f64 / 2048.0 / 1024.0,
            max_used_disk_gb: maximum_disk_usage.load(Ordering::Relaxed) as f64
                / (1024.0 * 1024.0 * 1024.0),
            output_file_sizes: WalkDir::new(out_dir_for_final_size)
                .into_iter()
                .filter_map(|p| p.ok())
                .filter_map(|file| {
                    let metadata = file.metadata().ok()?;
                    let path = file.path();
                    let size = metadata.len();

                    Some((
                        path.to_string_lossy().into_owned(),
                        (size, size as f64 / (1024.0 * 1024.0)),
                    ))
                })
                .collect(),

            has_completed,
            timed_out,
            timeout_secs: parameters.timeout.map(|d| d.as_secs_f64()),
            deadlock_detected,
        }
    }
}
