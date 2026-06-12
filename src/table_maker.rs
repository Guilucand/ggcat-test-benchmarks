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
    #[structopt(long)]
    typst: bool,
    /// Comma-separated bash-like glob patterns matched against the raw tool
    /// name (e.g. "*colored*" or "ggcat,bifrost-colored"). When set, only
    /// tools matching at least one pattern are included.
    #[structopt(long)]
    tools: Option<String>,
    /// Invert --tools: keep only tools that match none of the patterns.
    #[structopt(long)]
    invert_tools: bool,
}

/// Match a bash-style glob with `*` (zero or more chars) and `?` (one char)
/// against `text`. Anchored to the full string.
fn glob_match(pattern: &str, text: &str) -> bool {
    let pat: Vec<char> = pattern.chars().collect();
    let txt: Vec<char> = text.chars().collect();
    // Iterative backtracking matcher.
    let (mut pi, mut ti) = (0usize, 0usize);
    let (mut star_pi, mut star_ti): (Option<usize>, usize) = (None, 0);
    while ti < txt.len() {
        if pi < pat.len() && (pat[pi] == '?' || pat[pi] == txt[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < pat.len() && pat[pi] == '*' {
            star_pi = Some(pi);
            star_ti = ti;
            pi += 1;
        } else if let Some(sp) = star_pi {
            pi = sp + 1;
            star_ti += 1;
            ti = star_ti;
        } else {
            return false;
        }
    }
    while pi < pat.len() && pat[pi] == '*' {
        pi += 1;
    }
    pi == pat.len()
}

struct TableMaker {
    cells: Vec<Vec<Option<(String, Option<String>, Option<String>)>>>,
    row_labels: Vec<(String, String)>,
    col_labels: Vec<String>,
}

const MULTIROW_ALIGNMENT: [f64; 4] = [0.0, -0.6, -1.3, -1.9];

const REMAPPINGS: &[(&str, &str)] = &[
    ("mother", "Human"),
    ("gut", "Gut microbiome"),
    ("salmonella-100k", "Salmonella archive (100K)"),
    ("salmonella-all", "Salmonella archive (309K)"),
    ("human-100", "100 Human genomes"),
    ("human-100", "100 Human genomes"),
    ("cuttlefish2", "Cuttlefish 2"),
    ("bifrost", "BiFrost"),
    ("bifrost-colored", "BiFrost colored"),
    ("ggcat", "GGCAT"),
    ("ggcat-colored", "GGCAT colored"),
];

impl TableMaker {
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
        values: (String, Option<String>, Option<String>),
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

    pub fn make_typst_table(&self, title: String) -> String {
        let mut buffer = String::new();

        assert!(self.col_labels.len() > 0);

        let col_count = self.col_labels.len();

        buffer.push_str("#figure(\n");
        buffer.push_str(&format!("\tcaption: [{}],\n", title));
        buffer.push_str("\ttable(\n");
        buffer.push_str(&format!("\t\tcolumns: {},\n", col_count + 2));

        buffer.push_str(&{
            let mut col_def = String::from("\t\talign: (left, center");
            for _ in 0..col_count {
                col_def.push_str(", center");
            }
            col_def.push_str("),\n");
            col_def
        });

        buffer.push_str(&{
            let mut col_names = String::from("\t\ttable.header([Dataset], [$k$]");
            for label in &self.col_labels {
                col_names.push_str(", [");
                col_names.push_str(&label.replace('#', "\\#"));
                col_names.push_str("]");
            }
            col_names.push_str("),\n");
            col_names
        });

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
                "\t\ttable.cell(rowspan: {}, [{}]),\n",
                subrows_count,
                // MULTIROW_ALIGNMENT[subrows_count - 1],
                dataset_name
            ));

            for row_idx in dataset_section.clone() {
                buffer.push_str(&{
                    let mut row_content = String::new();
                    row_content.push_str("\t\t[");
                    row_content.push_str(&self.row_labels[row_idx].1);
                    row_content.push_str("],");

                    for col_idx in 0..self.col_labels.len() {
                        row_content.push_str(&format!(
                            " [{} ({}) [{}]],",
                            self.cells[row_idx][col_idx]
                                .as_ref()
                                .map(|x| x.0.clone())
                                .unwrap_or(String::new()),
                            self.cells[row_idx][col_idx]
                                .as_ref()
                                .map(|x| x.1.as_ref().unwrap_or(&String::new()).clone())
                                .unwrap_or(String::new()),
                            self.cells[row_idx][col_idx]
                                .as_ref()
                                .map(|x| x.2.as_ref().unwrap_or(&String::new()).clone())
                                .unwrap_or(String::new()),
                        ));
                    }
                    row_content.push_str("\n");
                    // row_content
                    //     .push_str(&format!("\\\\\\cline{{2-{}}}\n", self.col_labels.len() + 2));
                    row_content
                });
            }

            // buffer.push_str("\\hline\n");
        }
        buffer.push_str("\t)\n");

        // buffer.push_str("\\end{tabular}\n");

        // buffer.push_str("\\label{fig:my_label}\n");
        buffer.push_str(")\n");

        buffer
    }

    pub fn make_latex_table(&self, title: String) -> String {
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
            let mut col_names = String::from(r#"Dataset&$k$"#);
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
                            "\\cell{{{} ({}) [{}]}}",
                            self.cells[row_idx][col_idx]
                                .as_ref()
                                .map(|x| x.0.clone())
                                .unwrap_or(String::new()),
                            self.cells[row_idx][col_idx]
                                .as_ref()
                                .map(|x| x.1.as_ref().unwrap_or(&String::new()).clone())
                                .unwrap_or(String::new()),
                            self.cells[row_idx][col_idx]
                                .as_ref()
                                .map(|x| x.2.as_ref().unwrap_or(&String::new()).clone())
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

        buffer.push_str("\\end{tabular}\n");

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

fn remap(val: &str) -> String {
    REMAPPINGS
        .iter()
        .find(|x| x.0 == val)
        .map(|x| x.1)
        .unwrap_or(val)
        .to_string()
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
        let tool = tool
            .strip_suffix(&format!("-k{}", k))
            .unwrap_or(&tool)
            .to_string();

        let dataset = dataset
            .strip_suffix(&format!("-{}", dataset))
            .unwrap_or(&dataset)
            .to_string();

        let threads: usize = parts[4][1..(parts[4].len() - "thr-info.json".len())]
            .parse()
            .unwrap();

        Some(Self {
            dataset,
            wdir,
            k,
            tool,
            threads,
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

    let mut table_maker = TableMaker::new();

    let tool_patterns: Option<Vec<String>> = args.tools.as_ref().map(|s| {
        s.split(',')
            .map(|p| p.trim().to_string())
            .filter(|p| !p.is_empty())
            .collect()
    });
    let invert_tools = args.invert_tools;
    let tool_matches = |tool: &str| -> bool {
        match &tool_patterns {
            None => true,
            Some(pats) => {
                let any = pats.iter().any(|p| glob_match(p, tool));
                if invert_tools {
                    !any
                } else {
                    any
                }
            }
        }
    };

    content.retain(|p| ParsedPath::from_path(p).is_some());
    content.sort_by_cached_key(|p| ParsedPath::from_path(p).unwrap());

    for target_dataset in args.datasets.split(",") {
        let start_row = table_maker.row_labels.len();

        for file in &content {
            if !file.ends_with("info.json") {
                continue;
            }

            let ParsedPath {
                dataset,
                wdir,
                k,
                tool,
                threads,
            } = ParsedPath::from_path(&file).unwrap();

            if dataset != target_dataset {
                continue;
            }

            if !tool_matches(&tool) {
                continue;
            }

            let results: RunResults = serde_json::from_reader(File::open(&file).unwrap()).unwrap();

            let hours = (results.real_time_secs / 3600.0) as usize;
            let minutes = ((results.real_time_secs / 60.0) % 60.0) as usize;
            let seconds = ((results.real_time_secs) % 60.00) as usize;

            let duration_string = if args.seconds_time {
                format!("{}h:{}m:{}s", hours, minutes, seconds)
            } else if hours == 0 {
                format!("{}m:{}s", minutes, seconds)
            } else {
                format!("{}h:{}m", hours, minutes)
            };

            let status_label = if results.deadlock_detected {
                Some("DLK")
            } else if results.timed_out {
                Some("TLE")
            } else if !results.has_completed {
                Some("crashed")
            } else {
                None
            };

            table_maker.add_sample(
                &remap(&dataset),
                &k.to_string(),
                &remap(&tool),
                match status_label {
                    None => (
                        duration_string,
                        Some(format!("{:.2}GB", results.max_memory_gb)),
                        Some(format!("{:.2}GB", results.max_used_disk_gb)),
                    ),
                    Some(label) => (label.to_string(), None, None),
                },
            );

            let total_output_bytes: u64 = results
                .output_file_sizes
                .iter()
                .map(|(_, (b, _))| *b)
                .sum();
            let total_output_gb =
                total_output_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
            let fasta_files: Vec<&String> = results
                .output_file_sizes
                .iter()
                .filter_map(|(p, _)| if p.ends_with(".fa") { Some(p) } else { None })
                .collect();

            println!(
                "{} {} K={} {} T={} => time={:.1}s, max_mem={:.2}GB, max_disk={:.2}GB, completed={}, timed_out={}, deadlock={}, total_output={:.2}GB, fasta_files={:#?}",
                dataset,
                wdir,
                k,
                tool,
                threads,
                results.real_time_secs,
                results.max_memory_gb,
                results.max_used_disk_gb,
                results.has_completed,
                results.timed_out,
                results.deadlock_detected,
                total_output_gb,
                fasta_files,
            );
        }

        if table_maker.row_labels.len() == start_row {
            println!("WARN: Dataset {} has no entries", target_dataset);
        }
    }

    println!(
        "Table: \n{}",
        if args.typst {
            table_maker.make_typst_table(args.title)
        } else {
            table_maker.make_latex_table(args.title)
        }
    );
}
