use anyhow::Result;
use colored::*;
use::std::{ fs, path, process, env, collections::HashMap };
use crate::parser::{ RemoteRegistryIndex, GitHubItem };
use crate::paths::{
    REMOTE_REGISTRY_INDEX
};

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

pub fn fetch_remote_registry_index() -> Result<HashMap<String, RemoteRegistryIndex>> {
    let client = reqwest::blocking::Client::new();
    let response = client
       .get(REMOTE_REGISTRY_INDEX)
       .header("User-Agent", "cspm")
       .send()?
       .error_for_status()?;

    let rjson: HashMap<String, RemoteRegistryIndex> = response.json()?;
    Ok(rjson)
}

fn download_from_remote_registry(url: &str, destination: &path::Path) -> Result<()> {
     let client = reqwest::blocking::Client::new();
     let response: Vec<GitHubItem> = client
        .get(url)
        .header("User-Agent", "cspm")
        .send()?
        .error_for_status()?
        .json()?;

     fs::create_dir_all(destination)?;

     for item in response {
         let item_destination = destination.join(item.name.clone());
         match item.r#type.as_str() {
             "file" => {
                 if let Some(down_url) = item.download_url {
                     let bytes_response = client
                         .get(down_url)
                         .header("User-Agent", "cspm")
                         .send()?
                         .error_for_status()?
                         .bytes()?;

                     fs::write(item_destination, &bytes_response)?;
                 }
             },
             "dir" => {
                 let sub_url = format!("{}/{}", url, item.name);
                 download_from_remote_registry(&sub_url, &item_destination)?;
             }
             _ => { continue }
         }
     }

     Ok(())
}

pub fn download_package(
    remote_registry_url: &str,
    pname: &str,
    version: &str,
    cache_path: &path::Path,
    dest_path: &path::Path
) -> Result<()>
{
    let remote_module_url = format!("{}/{}/{}", remote_registry_url, pname, version);

    log_message(
        MessageType::Info(format!("Download module {} from {}", pname, remote_registry_url)),
        Some("DOWNLOAD"),
        true
    );

    let destination = cache_path.join(dest_path);
    if let Err(e) = download_from_remote_registry(&remote_module_url, &destination) {
        let mes_err = log_message(
            MessageType::Error(format!("Failed to download module: {}", e)),
            Some("DOWNLOAD"),
            false
        );

        return Err(anyhow::anyhow!(mes_err))
    }

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
