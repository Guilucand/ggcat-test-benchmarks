use crate::RunResults;
use itertools::*;
use std::borrow::Borrow;
use std::cmp::max;
use std::fs::File;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::time::Duration;
use structopt::StructOpt;

#[derive(StructOpt)]
pub struct TableMakerCli {
    #[structopt(long, short)]
    datasets: String,
    results_dirs: Vec<PathBuf>,
    #[structopt(long, short)]
    title: String,
    #[structopt(long, short)]
    seconds_time: bool,
}

struct LatexTableMaker {
    cells: Vec<Vec<Option<(String, Option<String>)>>>,
    row_labels: Vec<(String, String)>,
    col_labels: Vec<String>,
}

const MULTIROW_ALIGNMENT: [f64; 4] = [0.0, -0.6, -1.3, -1.9];

impl LatexTableMaker {
    pub fn new() -> Self {
        Self {
            cells: vec![],
            row_labels: vec![],
            col_labels: vec![],
        }
    }

    fn find_el<B: ?Sized + Eq + ToOwned<Owned = T>, T: Borrow<B>>(
        vec: &mut Vec<T>,
        el: &B,
    ) -> usize {
        vec.iter()
            .position(|x| x.borrow() == el)
            .unwrap_or_else(|| {
                vec.push(el.to_owned());
                vec.len() - 1
            })
    }

    pub fn add_sample(
        &mut self,
        row: &str,
        sub_row: &str,
        col: &str,
        values: (String, Option<String>),
    ) {
        let row_idx = Self::find_el(
            &mut self.row_labels,
            &(row.to_string(), sub_row.to_string()),
        );
        let col_idx = Self::find_el(&mut self.col_labels, col);

        self.cells.resize(self.row_labels.len(), Vec::new());

        for row in &mut self.cells {
            row.resize(self.col_labels.len(), None);
        }

        self.cells[row_idx][col_idx] = Some(values);
    }

    pub fn make_table(&self, title: String) -> String {
        let mut buffer = String::new();

        assert!(self.col_labels.len() > 0);

        let col_count = self.col_labels.len();

        buffer.push_str("\\begin{table}\n");
        buffer.push_str("\\centering\n");
        buffer.push_str(&format!("\\caption{{{}}}\n", title));

        buffer.push_str(&{
            let mut col_def = String::from(r#"\begin{tabular}{ |c|c||c"#);
            for _ in 0..(col_count - 1) {
                col_def.push_str("|c");
            }
            col_def.push_str("| }\n");
            col_def
        });
        // buffer.push_str("\\hline\n");
        // buffer.push_str(&format!(
        //     "\\multicolumn{{{}}}{{|c|}}{{{}}}\\\\\n",
        //     col_count + 1,
        //     title
        // ));
        // buffer.push_str("\\hline\n");
        buffer.push_str("\\hline\n");
        buffer.push_str(&{
            let mut col_names = String::from(r#"Dataset&K"#);
            for label in &self.col_labels {
                col_names.push_str("&");
                col_names.push_str(label);
            }
            col_names.push_str("\\\\\n");
            col_names
        });
        buffer.push_str("\\hline\n");

        for (dataset_name, dataset_section) in self
            .row_labels
            .iter()
            .enumerate()
            .group_by(|x| x.1 .0.clone())
            .into_iter()
        {
            let dataset_section: Vec<_> = dataset_section.map(|d| d.0).collect();

            let subrows_count = dataset_section.len();
            buffer.push_str(&format!(
                "\\multirow{{{}}}{{*}}[{}em]{{{}}}",
                subrows_count,
                MULTIROW_ALIGNMENT[subrows_count - 1],
                dataset_name
            ));

            for row_idx in dataset_section.clone() {
                buffer.push_str(&{
                    let mut row_content = String::from("&");
                    row_content.push_str(&self.row_labels[row_idx].1);

                    for col_idx in 0..self.col_labels.len() {
                        row_content.push_str("&");
                        row_content.push_str(&format!(
                            "\\cell{{{}\\\\({})}}",
                            self.cells[row_idx][col_idx]
                                .as_ref()
                                .map(|x| x.0.clone())
                                .unwrap_or(String::new()),
                            self.cells[row_idx][col_idx]
                                .as_ref()
                                .map(|x| x.1.as_ref().unwrap_or(&String::new()).clone())
                                .unwrap_or(String::new()),
                        ));
                    }
                    row_content
                        .push_str(&format!("\\\\\\cline{{2-{}}}\n", self.col_labels.len() + 2));
                    row_content
                });
            }
            buffer.push_str("\\hline\n");
        }

        buffer.push_str(r#"\end{tabular}"#);

        // buffer.push_str("\\label{fig:my_label}\n");
        buffer.push_str("\\end{table}\n");

        buffer
    }
}

/*
*/

#[derive(Clone, Ord, PartialOrd, Eq, PartialEq)]
struct ParsedPath {
    dataset: String,
    wdir: String,
    k: usize,
    tool: String,
    threads: usize,
}

impl ParsedPath {
    pub fn from_path(path: &str) -> Option<Self> {
        if !path.ends_with("info.json") {
            return None;
        }

        let file_name = path.split("/").last().unwrap();
        let parts: Vec<_> = file_name.split("_").collect();

        // {}_{}_K{}_{}_T{}thr-info.json
        let dataset = parts[0].to_string();
        let wdir = parts[1].to_string();
        let k: usize = parts[2][1..].parse().unwrap();
        let tool = parts[3].to_string();

        let tool = tool.strip_suffix("-ref").unwrap_or(&tool);
        let tool = tool.strip_suffix("-reads").unwrap_or(&tool);
        let tool = tool.strip_suffix(&format!("-k{}", k)).unwrap_or(&tool).to_string();

        let threads: usize = parts[4][1..(parts[4].len() - "thr-info.json".len())]
            .parse()
            .unwrap();

        Some(Self {
            dataset,
            wdir,
            k,
            tool,
            threads
        })
    }
}

pub fn make_table(args: TableMakerCli) {
    let mut content: Vec<_> = args
        .results_dirs
        .iter()
        .map(|dir| {
            fs_extra::dir::get_dir_content(dir.join("results-dir"))
                .unwrap()
                .files
                .into_iter()
        })
        .flatten()
        .collect();

    let mut table_maker = LatexTableMaker::new();

    content.retain(|p| ParsedPath::from_path(p).is_some());
    content.sort_by_cached_key(|p| ParsedPath::from_path(p).unwrap());

    for target_dataset in args.datasets.split(",") {
        let start_row = table_maker.row_labels.len();

        for file in &content {
            if !file.ends_with("info.json") {
                continue;
            }

            let ParsedPath {
                dataset, wdir, k, tool, threads
            } = ParsedPath::from_path(&file).unwrap();

            if dataset != target_dataset {
                continue;
            }

            let results: RunResults = serde_json::from_reader(File::open(&file).unwrap()).unwrap();

            let hours = ((results.real_time_secs / 3600.0)) as usize;
            let minutes = ((results.real_time_secs / 60.0) % 60.0) as usize;
            let seconds = ((results.real_time_secs) % 60.00) as usize;

            let duration_string = if args.seconds_time {
                format!("{}h:{}m:{}s", hours, minutes, seconds)
            } else {
                format!("{}h:{}m", hours, minutes)
            };

            table_maker.add_sample(
                &dataset,
                &k.to_string(),
                &tool,
                if results.has_completed {
                    (
                        duration_string,
                        Some(format!("{:.2}gb", results.max_memory_gb)),
                    )
                } else {
                    ("crashed".to_string(), None)
                },
            );

            println!(
                "{} {} {} {} {} => {:#?}",
                dataset, wdir, k, tool, threads, results
            );
        }

        if table_maker.row_labels.len() == start_row {
            println!("WARN: Dataset {} has no entries", target_dataset);
        }
    }

    println!(
        "Table: \n{}",
        table_maker.make_table(args.title)
    );
}
