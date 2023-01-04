//

use serde::{Deserialize, Serialize};
use std::env::current_dir;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use crate::{config::LocalConfig, parse_toml, DatasetStatsCli};
use fastq::Record;
use rayon::prelude::*;
use std::cmp::max;
use std::fs::File;
use std::io::BufRead;
use std::io::BufReader;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug)]
pub struct ExperimentStats {
    k: usize,
    dataset_kmer_count: u64,
    kmer_unique_count: u64,
    unitig_bases_count: u64,
    unitigs_count: u64,
    avg_unitig_length: f64,
    max_unitig_length: u64,
    unitigs_n50: u64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DatasetStats {
    dataset_name: String,
    files_count: usize,
    files_size: u64,
    bases_count: u64,
    sequences_count: u64,
    stats: Vec<ExperimentStats>,
}

pub fn compute_dataset_stats(args: DatasetStatsCli) {
    let base_dir = if args.env_config.is_absolute() {
        args.env_config.parent().unwrap().to_path_buf()
    } else {
        current_dir()
            .unwrap()
            .join(&args.env_config)
            .parent()
            .unwrap()
            .to_path_buf()
    };

    let local_env = parse_toml::<LocalConfig>(args.env_config);
    let dataset = local_env
        .datasets
        .into_iter()
        .filter(|d| d.name == args.dataset)
        .next()
        .unwrap();

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

    let files_count = AtomicU64::new(0);
    let files_size = AtomicU64::new(0);
    let bases_count = AtomicU64::new(0);
    let sequences_count = AtomicU64::new(0);

    let kmer_sizes = args
        .experiments
        .iter()
        .map(|x| {
            let raw_path = x.to_str().unwrap();
            let pos = raw_path.find("_K").unwrap();
            let k = raw_path[pos + 2..]
                .split('_')
                .next()
                .unwrap()
                .parse::<usize>()
                .unwrap();
            k
        })
        .collect::<Vec<_>>();
    let kmer_counts = kmer_sizes
        .iter()
        .map(|_| AtomicU64::new(0))
        .collect::<Vec<_>>();

    if let Some(tarball) = &dataset.tar {
        println!("Unpacking tarball: {}", tarball.display());
        for entry in tar::Archive::new(File::open(tarball).unwrap())
            .entries()
            .unwrap()
            .filter(|d| d.is_ok() && d.as_ref().unwrap().path().unwrap().extension().is_some())
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

            let dest_file = PathBuf::from("working-dirs/hdd").join(file_name);

            entry.unpack(&dest_file).unwrap();
            input_files.push(dest_file);
        }
    }

    let logging_steps = (input_files.len() / 20) + 1;

    println!("Starting processing...");

    input_files.into_par_iter().for_each(|file| {
        let mut update_counters = |seq: &[u8]| {
            bases_count.fetch_add(seq.len() as u64, Ordering::Relaxed);
            sequences_count.fetch_add(1, Ordering::Relaxed);

            for (i, size) in kmer_sizes.iter().enumerate() {
                kmer_counts[i].fetch_add(
                    if seq.len() >= *size {
                        (seq.len() - *size + 1) as u64
                    } else {
                        0
                    },
                    Ordering::Relaxed,
                );
            }
        };

        files_size.fetch_add(file.metadata().unwrap().len(), Ordering::Relaxed);

        let tmp_file = file.to_str().unwrap();

        if tmp_file.contains(".fq") || tmp_file.contains(".fastq") {
            fastq::parse_path(Some(file), |reader| {
                reader.each(|record| {
                    update_counters(record.seq());
                    true
                });
            });
        } else {
            fasta::read::FastaReader::new(&file).for_each(|[_, seq]| {
                update_counters(seq.as_bytes());
            });
        }

        let count = files_count.fetch_add(1, Ordering::Relaxed) as usize;
        if count % logging_steps == 0 {
            println!("Processed {} files for dataset {}", count, dataset.name);
        }
    });

    let stats = args
        .experiments
        .par_iter()
        .zip(kmer_sizes.par_iter())
        .enumerate()
        .map(|(i, (experiment, k))| {
            let mut experiment_result = ExperimentStats {
                k: *k,
                dataset_kmer_count: kmer_counts[i].load(Ordering::Relaxed),
                kmer_unique_count: 0,
                unitig_bases_count: 0,
                unitigs_count: 0,
                avg_unitig_length: 0.0,
                max_unitig_length: 0,
                unitigs_n50: 0,
            };

            let mut unitig_sizes = vec![];

            fasta::read::FastaReader::new(&experiment).for_each(|[_, seq]| {
                experiment_result.kmer_unique_count += if seq.len() >= *k {
                    (seq.len() - *k + 1) as u64
                } else {
                    0
                };
                experiment_result.unitig_bases_count += seq.len() as u64;
                experiment_result.unitigs_count += 1;
                experiment_result.max_unitig_length =
                    max(experiment_result.max_unitig_length, seq.len() as u64);
                unitig_sizes.push(seq.len() as u64);
            });

            unitig_sizes.sort_unstable();
            experiment_result.unitigs_n50 = unitig_sizes[(unitig_sizes.len() + 1) / 2];
            experiment_result.avg_unitig_length = experiment_result.unitig_bases_count as f64
                / experiment_result.unitigs_count as f64;

            experiment_result
        })
        .collect::<Vec<_>>();

    let final_results = DatasetStats {
        dataset_name: dataset.name.clone(),
        files_count: files_count.into_inner() as usize,
        files_size: files_size.into_inner(),
        bases_count: bases_count.into_inner(),
        sequences_count: sequences_count.into_inner(),
        stats,
    };

    println!("{}", serde_json::to_string_pretty(&final_results).unwrap());

    File::create(&format!("{}-stats.json", dataset.name))
        .unwrap()
        .write_all(
            serde_json::to_string_pretty(&final_results)
                .unwrap()
                .as_bytes(),
        )
        .unwrap();
}
