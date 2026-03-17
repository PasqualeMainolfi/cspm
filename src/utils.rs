use anyhow::Result;
use colored::*;
use regex::Regex;
use std::{ fs, path, process, env, collections::HashMap };
use crate::confres::ProjectPaths;
use crate::{ colored_name, cmd_exists };
use crate::parser::{ RemoteRegistryIndex, GitHubItem };


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

pub fn fetch_remote_registry_index(registry_url: &str) -> Result<HashMap<String, RemoteRegistryIndex>> {
    let client = reqwest::blocking::Client::new();
    let response = client
       .get(registry_url)
       .header("User-Agent", "cspm")
       .send()?
       .error_for_status()?;

    let rjson: HashMap<String, RemoteRegistryIndex> = response.json()?;
    Ok(rjson)
}

pub fn download_from_remote_registry(url: &str, destination: &path::Path) -> Result<()> {
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
             _ => continue
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
        MessageType::Info(format!("Download module {} from {}", colored_name!(pname), remote_registry_url)),
        Some("DOWNLOAD"),
        true
    );

    let destination = cache_path.join(dest_path);
    if let Err(e) = download_from_remote_registry(&remote_module_url, &destination) {
        let mes_err = log_message(
            MessageType::Error(format!("Failed to download module:\n{}", e)),
            Some("DOWNLOAD"),
            false
        );

        return Err(anyhow::anyhow!(mes_err))
    }

    Ok(())
}

pub fn get_csound_version() -> Option<String> {
    let cmd = process::Command::new("csound").arg("--version").output().ok()?;
    let stdout_string = String::from_utf8_lossy(&cmd.stdout);
    let stderr_string = String::from_utf8_lossy(&cmd.stderr);
    let vstring = format!("{}{}", stdout_string, stderr_string);
    let re = Regex::new(r"\d+\.\d+(\.\d+)?").unwrap();

    if let Some(v) = re.find(&vstring) {
        return Some(v.as_str().to_string())
    }

    log_message(MessageType::Warning("Csound version not found".to_string()), None, true);
    None
}

pub fn check_csound_installed() -> Option<String> {
    if !cmd_exists!("csound") {
        log_message(
            MessageType::Warning(
                "Csound executable not found. Please install csound from <https://github.com/csound/csound/releases> and specify the Csound version in Cspm.toml file".to_string()
            ),
            None,
            true
        );
        None
    } else {
        get_csound_version()
    }
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
            MessageType::Error(format!("Csound exited with non-zero status:\n{}", status)),
            Some("RUN"), false
        );
        return Err(anyhow::anyhow!(mes_err))
    }

    Ok(())
}

pub fn check_risset() -> Result<()> {
    // check if risset is installed
    let risset_exists = cmd_exists!("risset");

    if !risset_exists {
        log_message(MessageType::Info("risset not found. Installing...".to_string()), Some("RISSET"), true);
        // check if uv is installed
        let uv_exists = cmd_exists!("uv");
        if !uv_exists {
            match env::consts::OS {
                "linux" | "macos" => {
                    log_message(MessageType::Info("Install uv".to_string()), Some("RISSET"), true);
                    process::Command::new("sh")
                        .arg("-c")
                        .arg("curl -LsSf https://astral.sh/uv/install.sh | sh")
                        .status()?;
                },
                "windows" => {
                    log_message(MessageType::Info("Install uv".to_string()), Some("RISSET"), true);
                    process::Command::new("powershell")
                        .args([
                            "-ExecutionPolicy",
                            "ByPass",
                            "-c",
                            "irm https://astral.sh/uv/install.ps1 | iex"
                        ])
                        .status()?;
                },
                _ => {
                    let mes_err = log_message(MessageType::Error("Unknown OS".to_string()), Some("RISSET"), false);
                    return Err(anyhow::anyhow!(mes_err))
                }
            }
        }

        log_message(MessageType::Info("Install risset".to_string()), Some("RISSET"), true);

        // install risset
        process::Command::new("uv")
            .args(["tool", "install", "risset"])
            .status()?;

        log_message(MessageType::Info("risset has been installed".to_string()), Some("RISSET"), true);
    }

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
                format!("Plugins installation exited with non-zero status:\n{}", status)
            ),
            Some("RISSET"),
            false
        );

        return Err(anyhow::anyhow!(mes_err))
    }

    Ok(())
}

pub fn check_gitignore(paths: &ProjectPaths) -> Result<()> {
    let mut flag = false;
    if !paths.gitignore_file.exists() {
        log_message(
            MessageType::Error(
                ".gitignore file not found. Create it and add cs_modules and .config.toml to prevent them from being committed.".to_string()
            ),
            None,
            true
        );
        flag = true;
    } else {
        let gitignore_content = fs::read_to_string(&paths.gitignore_file)?;
        let text = gitignore_content
            .lines()
            .map(str::trim)
            .filter(|l| !l.starts_with('#'))
            .collect::<Vec<&str>>();

        let has_csmod = text.iter().any(|l| *l == "cs_modules" || *l == "/cs_modules");
        let has_prj = text.iter().any(|l| *l == ".config.toml");

        if !has_csmod && paths.modules_folder.exists() {
            log_message(
                MessageType::Error(
                    "The cs_modules directory exists in this project. Add it to .gitignore".to_string()
                ),
                None,
                true
            );
            flag = true;
        }

        if !has_prj && paths.project_info_file.exists() {
            log_message(
                MessageType::Error(
                    "The .config.toml file exists in this project. Add it to .gitignore".to_string()
                ),
                None,
                true
            );
            flag = true;
        }
    }

    if flag { return Err(anyhow::anyhow!("Invalid .gitignore file")) }
    Ok(())
}
