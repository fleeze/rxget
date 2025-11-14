use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = "多线程下载工具。")]
pub struct Args {
  // 下载地址
  #[arg(short, long)]
  pub url: String,

  // 线程数
  #[arg(short, long, default_value_t = 4)]
  pub thread: u64,

  #[arg(short, long)]
  pub output: Option<String>,
}

impl Args {
  pub fn parse_args() -> Args {
    Args::parse()
  }
}