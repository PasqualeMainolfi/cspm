use flate2::read::GzDecoder;
use tar::Archive;
use anyhow::Result;
use colored::*;
use::std::{ path, process, env };


pub enum MessageType {
    Info(String),
    Warning(String),
    Error(String)
}

pub fn log_message(mtype: MessageType, context: Option<&str>, display: bool) -> String {
    let cont = match context {
        Some(c) => format!("::{}", c),
        None => "".to_string()
    };

    let m = match mtype {
        MessageType::Info(m) => {
            let mtype = format!("[INFO{}]", cont);
            format!("{} {}", mtype.white().bold(), m)
        }
        MessageType::Warning(m) => {
            let mtype = format!("[WARNING{}]", cont);
            format!("{} {}", mtype.yellow().bold(), m)
        }
        MessageType::Error(m) => {
            let mtype = format!("[ERROR{}]", cont);
            format!("{} {}", mtype.red().bold(), m)
        }
    };

    if display { println!("{}", m); }
    return m;
}

pub fn download_package(registry: &str, pname: &str, version: &str, cache_path: &path::Path, dest_path: &path::Path) -> Result<()> {
    let url = format!("{}/{}-{}.tar.gz", registry, pname, version); // should be tar.gz

    log_message(
        MessageType::Info(format!("Download package {} from {}", pname, registry)),
        Some("DOWNLOAD"),
        true
    );

    let mut response = reqwest::blocking::get(&url)?;
    let temp_file_path = cache_path.join(format!("{}_temp.tar.gz", pname));

    {
        let mut temp_file = std::fs::File::create(&temp_file_path)?;
        std::io::copy(&mut response, &mut temp_file)?;
    }

    log_message(MessageType::Info("Unpack module".to_string()), Some("DOWNLOAD"), true);

    let tar_gz = std::fs::File::open(&temp_file_path)?;
    let decoder = GzDecoder::new(&tar_gz);
    let mut archive = Archive::new(decoder);
    archive.unpack(cache_path.join(dest_path))?;

    log_message(MessageType::Info("Remove downloaded temp file".to_string()), Some("DOWNLOAD"), true);

    std::fs::remove_file(&temp_file_path)?;
    Ok(())
}

pub fn run_csound_script(entry_point: &(String, String), cs_options: &Vec<String>) -> Result<()> {
    let (file1, file2) = entry_point;
    let mut c = process::Command::new("csound");
    c.arg(file1);
    if !file2.is_empty() { c.arg(file2); }
    if !cs_options.is_empty() {
        for flag in cs_options.iter() {
            c.arg(flag);
        }
    }

    let status = c.status()?;
    if !status.success() {
        let mes_err = log_message(
            MessageType::Error(format!("Csound exited with non-zero status: {}", status)),
            Some("RUN"), false
        );
        return Err(anyhow::anyhow!(mes_err))
    }

    Ok(())
}

pub fn check_risset() -> Result<()> {
    // check if risset is installed
    if process::Command::new("risset").output().is_err() {

        log_message(MessageType::Info("risset not found. Installing...".to_string()), Some("RISSET"), true);

        match env::consts::OS {
            "linux" | "macos" => {
                log_message(MessageType::Info("Install uv".to_string()), Some("RISSET"), true);
                process::Command::new("curl")
                    .args(["-LsSf", "https://astral.sh/uv/install.sh", "|", "sh"])
                    .status()?;
            },
            "windows" => {
                log_message(MessageType::Info("Install uv".to_string()), Some("RISSET"), true);
                println!("[RISSET::INFO] Install uv");
                process::Command::new("powershell")
                    .args(["-ExecutionPolicy", "ByPass"])
                    .args(["-c", "irm https://astral.sh/uv/install.ps1", "|", "iex"])
                    .status()?;
            },
            _ => {
                let mes_err = log_message(MessageType::Error("Unknown OS".to_string()), Some("RISSET"), false);
                return Err(anyhow::anyhow!(mes_err))
            }
        }

        log_message(MessageType::Info("Install risset".to_string()), Some("RISSET"), true);

        process::Command::new("uv")
            .arg("tool")
            .args(["install", "risset"])
            .status()?;

        log_message(MessageType::Info("Upgrade risset".to_string()), Some("RISSET"), true);

        process::Command::new("uv")
            .arg("tool")
            .args(["upgrade", "risset"])
            .status()?;
    }

    log_message(MessageType::Info("risset has been installed".to_string()), Some("RISSET"), true);

    Ok(())
}

pub fn run_risset(rst_options: &Vec<String>) -> Result<()> {
    let mut c = process::Command::new("risset");
    if !rst_options.is_empty() {
        for flag in rst_options.iter() {
            c.arg(flag);
        }
    }

    let status = c.status()?;
    if !status.success() {
        let mes_err = log_message(
            MessageType::Error(
                format!("Plugins installation exited with non-zero status: {}", status)
            ),
            Some("RISSET"),
            false
        );

        return Err(anyhow::anyhow!(mes_err))
    }

    Ok(())
}
