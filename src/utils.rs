use anyhow::Result;
use colored::*;
use regex::Regex;
use std::{ fs, process, env };
use crate::confres::ProjectPaths;
use crate::cmd_exists;


pub enum LogMessageType {
    Info(String),
    Warning(String),
    Error(String)
}

pub fn log_message(mtype: LogMessageType, context: Option<&str>, display: bool) -> String {
    let cont = match context {
        Some(c) => format!("::{}", c),
        None => "".to_string()
    };

    let m = match mtype {
        LogMessageType::Info(m) => {
            let mtype = format!("[INFO{}]", cont);
            format!("{} {}", mtype.white().bold(), m)
        }
        LogMessageType::Warning(m) => {
            let mtype = format!("[WARNING{}]", cont);
            format!("{} {}", mtype.yellow().bold(), m)
        }
        LogMessageType::Error(m) => {
            let mtype = format!("[ERROR{}]", cont);
            format!("{} {}", mtype.red().bold(), m)
        }
    };

    if display { println!("{}", m); }
    return m;
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

    log_message(LogMessageType::Warning("Csound version not found".to_string()), None, true);
    None
}

pub fn check_csound_installed() -> Option<String> {
    if !cmd_exists!("csound") {
        log_message(
            LogMessageType::Warning(
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
            LogMessageType::Error(format!("Csound exited with non-zero status:\n{}", status)),
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
        log_message(LogMessageType::Info("risset not found. Installing...".to_string()), Some("RISSET"), true);
        // check if uv is installed
        let uv_exists = cmd_exists!("uv");
        if !uv_exists {
            match env::consts::OS {
                "linux" | "macos" => {
                    log_message(LogMessageType::Info("Install uv".to_string()), Some("RISSET"), true);
                    process::Command::new("sh")
                        .arg("-c")
                        .arg("curl -LsSf https://astral.sh/uv/install.sh | sh")
                        .status()?;
                },
                "windows" => {
                    log_message(LogMessageType::Info("Install uv".to_string()), Some("RISSET"), true);
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
                    let mes_err = log_message(LogMessageType::Error("Unknown OS".to_string()), Some("RISSET"), false);
                    return Err(anyhow::anyhow!(mes_err))
                }
            }
        }

        log_message(LogMessageType::Info("Install risset".to_string()), Some("RISSET"), true);

        // install risset
        process::Command::new("uv")
            .args(["tool", "install", "risset"])
            .status()?;

        log_message(LogMessageType::Info("risset has been installed".to_string()), Some("RISSET"), true);
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
            LogMessageType::Error(
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
            LogMessageType::Error(
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
                LogMessageType::Error(
                    "The cs_modules directory exists in this project. Add it to .gitignore".to_string()
                ),
                None,
                true
            );
            flag = true;
        }

        if !has_prj && paths.project_info_file.exists() {
            log_message(
                LogMessageType::Error(
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
