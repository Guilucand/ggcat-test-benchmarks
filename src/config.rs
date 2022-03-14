use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Tool {
    pub name: String,
    pub path: PathBuf,
    pub arguments: String,

    #[serde(rename = "reads-arg-prefix")]
    pub reads_arg_prefix: Option<String>,
    #[serde(rename = "sequences-arg-prefix")]
    pub sequences_arg_prefix: Option<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct WorkingDir {
    pub name: String,
    pub path: PathBuf,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Dataset {
    pub name: String,
    pub files: Option<Vec<PathBuf>>,
    pub lists: Option<Vec<PathBuf>>,
    pub tar: Option<PathBuf>,
    pub limit: Option<usize>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Benchmark {
    pub name: String,
    pub datasets: Vec<String>,
    pub tools: Vec<String>,
    #[serde(rename = "working-dirs")]
    pub working_dirs: Vec<String>,
    #[serde(rename = "copy-dataset")]
    pub copy_dataset: bool,
    #[serde(rename = "trim-before")]
    pub trim_before: bool,
    #[serde(rename = "keep-temp")]
    pub keep_temp: Option<bool>,
    pub kvalues: Vec<usize>,
    pub threads: Vec<usize>,
    #[serde(rename = "max-memory")]
    pub max_memory: f64,
    #[serde(rename = "min-multiplicity")]
    pub min_multiplicity: usize,
    #[serde(rename = "size-check-time")]
    pub size_check_time: u64,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Config {
    pub tools: Vec<Tool>,
    pub datasets: Vec<Dataset>,
    pub benchmarks: Vec<Benchmark>,
    #[serde(rename = "working-dirs")]
    pub working_dirs: Vec<WorkingDir>,
}
