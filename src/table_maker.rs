use crate::RunResults;
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
/*
\begin{tabular}{ |p{3cm}||p{3cm}|p{3cm}|p{3cm}|  }
 \hline
 \multicolumn{4}{|c|}{Country List} \\
 \hline
 Country Name or Area Name& ISO ALPHA 2 Code &ISO ALPHA 3 Code&ISO numeric Code\\
 \hline
 Afghanistan   & AF    &AFG&   004\\
 Aland Islands&   AX  & ALA   &248\\
 Albania &AL & ALB&  008\\
 Algeria    &DZ & DZA&  012\\
 American Samoa&   AS  & ASM&016\\
 Andorra& AD  & AND   &020\\
 Angola& AO  & AGO&024\\
 \hline
\end{tabular}

*/

pub fn make_table(args: TableMakerCli) {
    let content = fs_extra::dir::get_dir_content(args.results_dir.join("results-dir")).unwrap();
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

        let results: RunResults = serde_json::from_reader(File::open(&file).unwrap()).unwrap();

        println!(
            "{} {} {} {} {} => {:#?}",
            dataset, wdir, k, tool, threads, results
        );
    }
}
