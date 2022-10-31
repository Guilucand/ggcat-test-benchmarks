use crate::config::{Dataset, Tool};
use fork::Fork;
use rlimit::Resource;

use crate::stats::get_process_info;
use cgroups_rs::cgroup_builder::*;
use cgroups_rs::*;
use serde::{Deserialize, Serialize};
use std::cmp::min;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
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
    pub has_completed: bool,
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

        let mut command = std::process::Command::new(&tool_path)
            .args(arguments.as_slice())
            .stdout(File::create(&parameters.log_file).unwrap())
            .stderr(File::create(parameters.log_file.with_extension("stderr")).unwrap())
            .spawn()
            .unwrap();

        let is_finished = Arc::new(AtomicBool::new(false));

        let pid = command.id();

        let is_finished_thr = is_finished.clone();
        let temp_dir_thr = parameters.temp_dir.clone();
        let out_dir_thr = PathBuf::from(&parameters.output_file)
            .parent()
            .unwrap()
            .to_path_buf();

        let maximum_disk_usage = Arc::new(AtomicU64::new(0));
        let maximum_rss_usage = Arc::new(AtomicU64::new(0));

        let maximum_disk_usage_thr = maximum_disk_usage.clone();
        let maximum_rss_usage_thr = maximum_rss_usage.clone();

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
        maximum_disk_usage_thread.join();

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
                canonical_kmers::canonicalize(&result, parameters.canonical_file, parameters.k);
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
            has_completed,
        }
    }
}
