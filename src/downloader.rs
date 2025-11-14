use reqwest::{Url, blocking::Client};

use tempfile::tempdir;

use crate::args::Args;
use std::{
    error::Error, fs::{self, File}, io::{self, Read, Write}, path::Path, sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    }, thread, time::{Duration, Instant}
};

fn ensure_parent_dir(path: &str) -> std::io::Result<()> {
  if let Some(parent) = Path::new(path).parent() {
      fs::create_dir_all(parent)?;
  }
  Ok(())
}

pub fn download_file(args: Args) -> Result<(), Box<dyn Error>> {
    let thread_num = args.thread;
    let url = args.url;
    let output = match args.output {
        Some(o) => o,
        None => {
            let parsed_url = Url::parse(&url).unwrap();
            let file_name = parsed_url
                .path_segments()
                .and_then(|segments| segments.last())
                .filter(|name| !name.is_empty())
                .ok_or_else(|| format!("无法从 {} 提取文件名，可使用 --output 指定文件名", url))?; // 默认文件名
            file_name.to_string()
        }
    };

    let client = Client::new();
    let resp = client.head(&url).send().unwrap();
    let total_size = resp
        .headers()
        .get(reqwest::header::CONTENT_LENGTH)
        .expect("获取CONTENT_LENGTH失败")
        .to_str()
        .unwrap()
        .parse::<u64>()
        .unwrap();
    println!("文件大小: {:.2} MB", total_size as f64 / 1024.0 / 1024.0);

    // let

    let chunk_size = total_size / thread_num;

    let downloaded = Arc::new(
        (0..thread_num)
            .map(|_| AtomicU64::new(0))
            .collect::<Vec<_>>(),
    );
    let mut handles = vec![];
    let start = Instant::now();

    let tmp_dir = Arc::new(tempdir().unwrap());
    println!("临时文件夹: {}", tmp_dir.path().display());

    for i in 0..thread_num {
        let range_start = i * chunk_size;
        let range_end = if i == thread_num - 1 {
            total_size - 1
        } else {
            (i + 1) * chunk_size - 1
        };
        let url = url.to_string();

        // 每个线程对应一个临时文件
        let tmp_dir = Arc::clone(&tmp_dir);
        let tmp_file = tmp_dir.path().join(format!("output_{}.tmp", i));
        let downloaded = Arc::clone(&downloaded);

        handles.push(thread::spawn(move || {
            let client = Client::builder()
                .timeout(Duration::from_secs(300))
                .build()
                .unwrap();
            let mut resp = client
                .get(&url)
                .header("Range", format!("bytes={}-{}", range_start, range_end))
                .send()
                .unwrap();

            let mut f = File::create(&tmp_file).unwrap();
            let mut buf = [0u8; 8192];

            loop {
                let n = resp.read(&mut buf).unwrap();
                if n == 0 {
                    break;
                }
                f.write_all(&buf[..n]).unwrap();
                downloaded[i as usize].fetch_add(n as u64, Ordering::SeqCst);
            }
        }));
    }

    let downloaded = Arc::clone(&downloaded);
    // 打印下载进度
    let progress_handle = thread::spawn(move || {
        loop {
            // 打印线程下载进度
            for i in 0..thread_num {
                let downloaded = downloaded[i as usize].load(Ordering::SeqCst);
                let progress = (downloaded as f64 / chunk_size as f64) * 100.0;
                println!(
                    "\r线程 {} 进度: {:.2}% ({:.2}/{:.2} MB)",
                    i,
                    progress,
                    downloaded as f64 / 1024.0 / 1024.0,
                    chunk_size as f64 / 1024.0 / 1024.0
                );
            }

            // 求和
            let downloaded = downloaded
                .iter()
                .map(|x| x.load(Ordering::SeqCst))
                .sum::<u64>();
            let progress = (downloaded as f64 / total_size as f64) * 100.0;
            println!(
                "\r下载进度: {:.2}% ({:.2}/{:.2} MB)",
                progress,
                downloaded as f64 / 1024.0 / 1024.0,
                total_size as f64 / 1024.0 / 1024.0
            );
            io::stdout().flush().unwrap();
            if downloaded >= total_size {
                break;
            }
            thread::sleep(Duration::from_millis(200));
            // 光标回到线程进度开始行
            print!("\x1B[{}A", thread_num + 1);
        }
    });

    // 等待所有线程完成
    for h in handles {
        h.join().unwrap();
    }

    progress_handle.join().unwrap();

    ensure_parent_dir(&output)?;

    // 合并临时文件
    let mut output = File::create(output).unwrap();
    for i in 0..thread_num {
        let tmp_dir = Arc::clone(&tmp_dir);
        let tmp_file = tmp_dir.path().join(format!("output_{}.tmp", i));
        let mut f = File::open(&tmp_file).unwrap();
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).unwrap();
        output.write_all(&buf).unwrap();
    }

    let duration = start.elapsed();
    println!("总耗时: {:?}", duration);

    Ok(())
}
