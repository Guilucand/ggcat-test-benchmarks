use crate::RunResults;
use std::cmp::max;
use std::fs::File;
use std::path::PathBuf;
use structopt::StructOpt;

#[derive(StructOpt)]
pub struct TableMakerCli {
    #[structopt(long, short)]
    filter: String,
    #[structopt(long, short)]
    rowlabel: String,
    #[structopt(long, short)]
    collabel: String,

    results_dir: PathBuf,
}

struct LatexTableMaker {
    cells: Vec<Vec<Option<(String, Option<String>)>>>,
    row_labels: Vec<String>,
    col_labels: Vec<String>,
}

impl LatexTableMaker {
    pub fn new() -> Self {
        Self {
            cells: vec![],
            row_labels: vec![],
            col_labels: vec![],
        }
    }

    fn find_el(vec: &mut Vec<String>, el: &str) -> usize {
        vec.iter().position(|x| x == el).unwrap_or_else(|| {
            vec.push(el.to_string());
            vec.len() - 1
        })
    }

    pub fn add_sample(&mut self, row: &str, col: &str, values: (String, Option<String>)) {
        let row_idx = Self::find_el(&mut self.row_labels, row);
        let col_idx = Self::find_el(&mut self.col_labels, col);

        self.cells.resize(self.row_labels.len(), Vec::new());

        for row in &mut self.cells {
            row.resize(self.col_labels.len(), None);
        }

        self.cells[row_idx][col_idx] = Some(values);
    }

    pub fn make_table(&self, title: String) -> String {
        let mut buffer = String::new();

        let col_count = self.col_labels.len();

        buffer.push_str("\\begin{figure}\n");
        buffer.push_str("\\centering\n");

        buffer.push_str(&{
            let mut col_def = String::from(r#"\begin{tabular}{ |c||c"#);
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
            let mut col_names = String::from(r#"Tool/K"#);
            for label in &self.col_labels {
                col_names.push_str("&");
                col_names.push_str(label);
            }
            col_names.push_str("\\\\\n");
            col_names
        });
        buffer.push_str("\\hline\n");

        for row_idx in 0..self.row_labels.len() {
            buffer.push_str(&{
                let mut row_content = self.row_labels[row_idx].clone();
                for col_idx in 0..self.col_labels.len() {
                    row_content.push_str("&");
                    row_content.push_str(&format!(
                        "\\makecell{{{}\\\\({})}}",
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
                row_content.push_str("\\\\\n");
                row_content
            });
            buffer.push_str("\\hline\n");
        }
        buffer.push_str(r#"\end{tabular}"#);


        buffer.push_str(&format!("\\caption{{{}}}\n", title));
        // buffer.push_str("\\label{fig:my_label}\n");
        buffer.push_str("\\end{figure}\n");

        // Afghanistan   & AF    &AFG&   004\\
        // Aland Islands&   AX  & ALA   &248\\
        // Albania &AL & ALB&  008\\
        // Algeria    &DZ & DZA&  012\\
        // American Samoa&   AS  & ASM&016\\
        // Andorra& AD  & AND   &020\\
        // Angola& AO  & AGO&024\\
        // \hline
        // \end{tabular}

        // format!("{:?}", self.cells)

        buffer
    }
}

/*
*/

pub fn make_table(args: TableMakerCli) {
    let mut content = fs_extra::dir::get_dir_content(args.results_dir.join("results-dir")).unwrap();

    let mut table_maker = LatexTableMaker::new();

    let target_dataset = args.filter;//"salmonella-10k";

    content.files.sort();

    for file in content.files {
        if !file.ends_with("info.json") {
            continue;
        }

        let file_name = file.split("/").last().unwrap();
        let parts: Vec<_> = file_name.split("_").collect();

        // {}_{}_K{}_{}_T{}thr-info.json
        let dataset = parts[0];
        let wdir = parts[1];
        let k: usize = parts[2][1..].parse().unwrap();
        let tool = parts[3];
        let threads: usize = parts[4][1..(parts[4].len() - "thr-info.json".len())]
            .parse()
            .unwrap();

        if dataset != target_dataset {
            continue;
        }

        let results: RunResults = serde_json::from_reader(File::open(&file).unwrap()).unwrap();


        let is_completed = {
            fs_extra::dir::get_dir_content(args.results_dir.join("outputs-dir")
                .join(&format!("{}_{}_K{}_{}_T{}thr_out", dataset, wdir, k, tool, threads))).map(|content| {
                let mut is_completed = false;
                for file in content.files {
                    if file.contains("canonical") {
                        is_completed = true;
                    }
                }
                is_completed
            }).unwrap_or(false)
        };

        table_maker.add_sample(
            &k.to_string(),
            tool,
            if is_completed {
                (
                    format!("{:.2}s", results.real_time_secs),
                    Some(format!("{:.2}gb", results.max_memory_gb)),
                )
            } else {
                (
                    "crashed".to_string(), None
                )
            },
        );

        println!(
            "{} {} {} {} {} => {:#?}",
            dataset, wdir, k, tool, threads, results
        );
    }

    println!(
        "Table: {}",
        table_maker.make_table(format!("Dataset: {}", target_dataset))
    );
}
