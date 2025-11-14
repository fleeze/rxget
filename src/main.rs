mod args;
use args::Args;

mod downloader;

use crate::downloader::download_file;

fn main() {
    let args = Args::parse_args();
    println!("{:?}", args);

    if let Err(e) = download_file(args) {
        eprintln!("下载失败: {}", e);
        std::process::exit(1);
    }
}
