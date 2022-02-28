pub mod config;
mod dir_cleanup;
pub mod runner;
mod table_maker;

use crate::config::Config;
use crate::dir_cleanup::{create_dir_with_guard, remove_dirs_on_panic};
use crate::runner::{Parameters, RunResults, Runner};
use crate::table_maker::{make_table, TableMakerCli};
use cgroups_rs::cgroup_builder::CgroupBuilder;
use cgroups_rs::Cgroup;
use std::env::current_dir;
use std::ffi::CString;
use std::fs::{create_dir, create_dir_all, read_dir, remove_dir_all, File};
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::panic;
use std::panic::resume_unwind;
use std::path::{Path, PathBuf};
use std::process::exit;
use std::time::Duration;
use structopt::*;

#[derive(StructOpt)]
enum ExtendedCli {
    #[cfg(feature = "cpu-limit")]
    Start(StartOpt),
    Bench(Cli),
    MakeTable(TableMakerCli),
    Canonicalize(CanonicalizeCli),
}

#[derive(StructOpt)]
struct StartOpt {}

#[derive(StructOpt)]
struct CanonicalizeCli {
    input: PathBuf,
    output: PathBuf,

    #[structopt(short, long)]
    kval: usize,

    #[structopt(short, long)]
    force: bool,
}

#[derive(StructOpt)]
struct Cli {
    test_name: String,
    results_path: PathBuf,
    #[structopt(short, long, default_value = "bench-settings.toml")]
    settings_file: PathBuf,
    #[structopt(long)]
    include: Option<String>,
    #[structopt(long)]
    exclude: Option<String>,
    #[structopt(long)]
    threads: Option<String>,
}

fn filter_options<T>(
    name: &str,
    options: Vec<T>,
    mapper: fn(&T) -> &String,
    include: &Vec<String>,
    exclude: &Vec<String>,
) -> Vec<T> {
    let include_matches = options.iter().any(|x| include.contains(mapper(x)));
    let exclude_matches = options.iter().any(|x| exclude.contains(mapper(x)));

    if include_matches && exclude_matches {
        println!(
            "Warning: both includes and excludes match on parameter: {}",
            name
        );
    }

    options
        .into_iter()
        .filter(|x| {
            if include_matches {
                include.contains(mapper(x))
            } else if exclude_matches {
                !exclude.contains(mapper(x))
            } else {
                true
            }
        })
        .collect()
}

fn main() {
    let args: ExtendedCli = ExtendedCli::from_args();

    ctrlc::set_handler(|| {
        panic!("Ctrl+C pressed, aborting!");
    })
    .unwrap();

    panic::set_hook(Box::new(move |info| {
        let stdout = std::io::stdout();
        let mut _lock = stdout.lock();

        let stderr = std::io::stderr();
        let mut err_lock = stderr.lock();

        let _ = writeln!(
            err_lock,
            "Thread panicked at location: {:?}",
            info.location()
        );
        let _ = writeln!(err_lock, "Error message: {}", info.to_string());
        if let Some(s) = info.payload().downcast_ref::<&str>() {
            let _ = writeln!(err_lock, "Panic payload: {:?}", s);
        }

        println!("Backtrace: {:?}", backtrace::Backtrace::new());

        remove_dirs_on_panic();

        exit(1);
    }));

    fdlimit::raise_fd_limit().unwrap();

    match args {
        #[cfg(feature = "cpu-limit")]
        ExtendedCli::Start(_opt) => {
            let hier = cgroups_rs::hierarchies::auto();
            let cg: Cgroup = CgroupBuilder::new("genome-benchmark-cgroup").build(hier);
            for subsys in cg.subsystems() {
                let path = subsys.to_controller().path();
                println!("Path: {}", path.display());

                let mut perms = std::fs::metadata(path).unwrap().permissions();
                perms.set_mode(0o777);
                std::fs::set_permissions(path, perms).unwrap();

                for path in std::fs::read_dir(path).unwrap() {
                    if let Ok(dir) = path {
                        let mut perms = std::fs::metadata(dir.path()).unwrap().permissions();
                        perms.set_mode(0o777);
                        std::fs::set_permissions(dir.path(), perms).unwrap();
                    }
                }
            }
        }
        ExtendedCli::Bench(args) => {
            let mut settings_file = File::open(&args.settings_file).unwrap();

            let base_dir = if args.settings_file.is_absolute() {
                args.settings_file.parent().unwrap().to_path_buf()
            } else {
                current_dir()
                    .unwrap()
                    .join(&args.settings_file)
                    .parent()
                    .unwrap()
                    .to_path_buf()
            };

            let mut settings_text = String::new();

            settings_file.read_to_string(&mut settings_text).unwrap();

            let results_dir = args.results_path.join("results-dir");
            let outputs_dir = args.results_path.join("outputs-dir");
            let logs_dir = args.results_path.join("logs-dir");

            std::fs::create_dir_all(&results_dir);
            std::fs::create_dir_all(&outputs_dir);
            std::fs::create_dir_all(&logs_dir);

            let settings: Config = toml::from_str(&settings_text).unwrap();

            let experiment = {
                let mut res = None;
                let mut multiple_choices = Vec::new();

                for bench in &settings.benchmarks {
                    if bench.name == args.test_name {
                        res = Some(bench.clone());
                    } else if bench.name.starts_with(&args.test_name) {
                        if res.is_none() {
                            res = Some(bench.clone());
                        } else {
                            if res.as_ref().unwrap().name != args.test_name {
                                multiple_choices.push(bench.name.clone());
                            }
                        }
                    }
                }

                if res.is_none() {
                    println!("Cannot find a benchmark matching \"{}\"!", args.test_name);
                    println!("Available benchmarks:");
                    for bench in settings.benchmarks {
                        println!("\t{}", &bench.name);
                    }
                    return;
                } else if multiple_choices.len() > 0 {
                    println!("Multiple benchmarks matching \"{}\"!", args.test_name);
                    println!("Matching benchmarks:");
                    println!("\t{}", res.as_ref().unwrap().name);
                    for bench in multiple_choices {
                        println!("\t{}", bench);
                    }
                    return;
                }
                res.unwrap()
            };

            let include = args
                .include
                .map(|i| i.split(",").map(|x| x.to_string()).collect::<Vec<_>>());
            let exclude = args
                .exclude
                .map(|i| i.split(",").map(|x| x.to_string()).collect::<Vec<_>>());

            let datasets = filter_options(
                "datasets",
                experiment
                    .datasets
                    .iter()
                    .map(|x| {
                        settings
                            .datasets
                            .iter()
                            .filter(|d| &d.name == x)
                            .next()
                            .expect(&format!("Cannot find a dataset with name '{}'", x))
                    })
                    .collect::<Vec<_>>(),
                |d| &d.name,
                include.as_ref().unwrap_or(&vec![]),
                exclude.as_ref().unwrap_or(&vec![]),
            );

            let tools = filter_options(
                "tools",
                experiment
                    .tools
                    .iter()
                    .map(|x| {
                        settings
                            .tools
                            .iter()
                            .filter(|t| &t.name == x)
                            .next()
                            .expect(&format!("Cannot find a tool with name '{}'", x))
                    })
                    .collect::<Vec<_>>(),
                |t| &t.name,
                include.as_ref().unwrap_or(&vec![]),
                exclude.as_ref().unwrap_or(&vec![]),
            );

            let mut working_dirs = filter_options(
                "working dirs",
                experiment.working_dirs,
                |x| &x,
                include.as_ref().unwrap_or(&vec![]),
                exclude.as_ref().unwrap_or(&vec![]),
            );

            for dataset in datasets {
                for working_dir in &working_dirs {
                    let working_dir = settings
                        .working_dirs
                        .iter()
                        .filter(|w| &w.name == working_dir)
                        .next()
                        .expect(
                            &format!("Cannot find a working dir named: {}", &working_dir).clone(),
                        );

                    let working_path = if working_dir.path.is_absolute() {
                        working_dir.path.clone()
                    } else {
                        base_dir.join(&working_dir.path)
                    };

                    let tmp_workdir = create_dir_with_guard(&working_path).expect(&format!(
                        "Cannot create working dir: {}",
                        working_path.display()
                    ));

                    let mut input_files: Vec<_> = dataset
                        .files
                        .as_ref()
                        .unwrap_or(&Vec::new())
                        .iter()
                        .map(|x| {
                            let path = if x.is_absolute() {
                                x.clone()
                            } else {
                                base_dir.join(x)
                            };
                            path
                        })
                        .collect();

                    if let Some(lists) = &dataset.lists {
                        for list in lists {
                            let list = if list.is_absolute() {
                                list.clone()
                            } else {
                                base_dir.join(list)
                            };

                            for line in BufReader::new(File::open(list).unwrap()).lines() {
                                input_files.push(PathBuf::from(line.unwrap()));
                            }
                        }
                    }

                    if let Some(limit) = dataset.limit {
                        input_files.truncate(limit);
                    }

                    if dataset.tar.is_some() && !experiment.copy_dataset {
                        println!("Warning: tar datasets must be copied to workdir (set copy-dataset = true)");
                        continue;
                    }

                    let mut dataset_copied = false;
                    let dataset_dir = tmp_workdir.as_ref().join("dataset");
                    create_dir(&dataset_dir);

                    if let Some(tarball) = &dataset.tar {
                        for entry in tar::Archive::new(File::open(tarball).unwrap())
                            .entries()
                            .unwrap()
                            .take(
                                dataset
                                    .limit
                                    .map(|limit| limit - input_files.len())
                                    .unwrap_or(usize::MAX),
                            )
                        {
                            let mut entry = entry.unwrap();
                            let tmp_path = entry.path().unwrap();
                            let file_name = tmp_path.file_name().unwrap();

                            let dest_file = dataset_dir.join(file_name);

                            entry.unpack(&dest_file).unwrap();
                            input_files.push(dest_file);
                        }
                    }

                    let threads = if let Some(threads) = &args.threads {
                        threads.split(",").map(|t| t.parse().unwrap()).collect()
                    } else {
                        experiment.threads.clone()
                    };

                    for thread in &threads {
                        for kval in &experiment.kvalues {
                            for tool in &tools {
                                let results_file = results_dir.join(&format!(
                                    "{}_{}_K{}_{}_T{}thr-info.json",
                                    dataset.name, working_dir.name, kval, tool.name, thread
                                ));

                                if results_file.exists() {
                                    println!(
                                        "File {} already exists, skipping test!",
                                        results_file.file_name().unwrap().to_str().unwrap()
                                    );
                                    continue;
                                }

                                if !dataset_copied && experiment.copy_dataset {
                                    dataset_copied = true;

                                    let mut new_input_files = Vec::new();

                                    for file in input_files {
                                        let name = Path::new(&file).file_name().unwrap();

                                        let new_file = dataset_dir.join(name);
                                        new_input_files.push(new_file.clone());

                                        if new_file == file {
                                            // Skip files already in dest dir
                                            continue;
                                        }

                                        std::fs::copy(&file, &new_file).expect(&format!(
                                            "Cannot copy file: {} to working dir {}",
                                            file.display(),
                                            new_file.display()
                                        ));
                                    }

                                    input_files = new_input_files
                                }

                                let temp_dir = tmp_workdir.as_ref().join(&format!(
                                    "{}_{}_K{}_{}_T{}thr_temp",
                                    dataset.name, working_dir.name, kval, tool.name, thread
                                ));
                                let out_dir = tmp_workdir.as_ref().join(&format!(
                                    "{}_{}_K{}_{}_T{}thr_out",
                                    dataset.name, working_dir.name, kval, tool.name, thread
                                ));
                                if temp_dir.exists()
                                    && temp_dir.read_dir().unwrap().next().is_some()
                                {
                                    panic!(
                                        "Temporary directory {} not empty!, aborting (file: {})",
                                        temp_dir.display(),
                                        temp_dir
                                            .read_dir()
                                            .unwrap()
                                            .next()
                                            .unwrap()
                                            .unwrap()
                                            .file_name()
                                            .into_string()
                                            .unwrap()
                                    );
                                }
                                if out_dir.exists() && out_dir.read_dir().unwrap().next().is_some()
                                {
                                    panic!(
                                        "Output directory {} not empty!, aborting",
                                        out_dir.display()
                                    );
                                }
                                create_dir_all(&temp_dir);
                                create_dir_all(&out_dir);

                                let results = Runner::run_tool(
                                    &base_dir,
                                    (*tool).clone(),
                                    dataset.name.clone(),
                                    &input_files,
                                    Parameters {
                                        max_threads: *thread,
                                        k: *kval,
                                        multiplicity: experiment.min_multiplicity,
                                        output_file: out_dir
                                            .join(&format!(
                                                "{}_{}_K{}_{}_T{}thr.fa",
                                                dataset.name,
                                                working_dir.name,
                                                kval,
                                                tool.name,
                                                thread
                                            ))
                                            .into_os_string()
                                            .into_string()
                                            .unwrap(),
                                        canonical_file: out_dir
                                            .join(&format!(
                                                "canonical_{}_{}_K{}_{}_T{}thr.fa",
                                                dataset.name,
                                                working_dir.name,
                                                kval,
                                                tool.name,
                                                thread
                                            ))
                                            .into_os_string()
                                            .into_string()
                                            .unwrap(),
                                        temp_dir: temp_dir
                                            .clone()
                                            .into_os_string()
                                            .into_string()
                                            .unwrap(),
                                        log_file: logs_dir.clone().join(&format!(
                                            "{}_{}_K{}_{}_T{}.log",
                                            dataset.name, working_dir.name, kval, tool.name, thread
                                        )),
                                        memory_gb: experiment.max_memory,
                                        size_check_time: Duration::from_millis(
                                            experiment.size_check_time,
                                        ),
                                    },
                                );

                                remove_dir_all(&temp_dir);

                                let final_out_dir = outputs_dir.join(&format!(
                                    "{}_{}_K{}_{}_T{}thr_out",
                                    dataset.name, working_dir.name, kval, tool.name, thread
                                ));
                                create_dir_all(&final_out_dir);

                                for file in read_dir(&out_dir).unwrap() {
                                    let file = file.unwrap();

                                    let name = file.file_name();
                                    std::fs::copy(file.path(), final_out_dir.join(name));
                                    std::fs::remove_file(file.path());
                                }
                                remove_dir_all(&out_dir);

                                File::create(results_file)
                                    .unwrap()
                                    .write_all(
                                        serde_json::to_string_pretty(&results).unwrap().as_bytes(),
                                    )
                                    .unwrap();
                            }
                        }
                    }
                }
            }
        }
        ExtendedCli::Canonicalize(args) => {
            if args.output.exists() && !args.force {
                println!("File {} already exists!", args.output.display());
            }

            canonical_kmers::canonicalize(args.input, args.output, args.kval);
        }
        ExtendedCli::MakeTable(args) => make_table(args),
    }
}
