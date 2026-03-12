use flate2::read::GzDecoder;
use tar::Archive;
use anyhow::Result;
use::std::{ path, process, env };

pub fn download_package(registry: &str, pname: &str, version: &str, cache_path: &path::Path, dest_path: &path::Path) -> Result<()> {
    let url = format!("{}/{}-{}.tar.gz", registry, pname, version); // should be tar.gz
    println!("[DOWNLOAD::INFO] Download package {} from {}", pname, registry);

    let mut response = reqwest::blocking::get(&url)?;
    let temp_file_path = cache_path.join(format!("{}_temp.tar.gz", pname));

    {
        let mut temp_file = std::fs::File::create(&temp_file_path)?;
        std::io::copy(&mut response, &mut temp_file)?;
    }

    println!("[DOWNLOAD::INFO] Unpack package");
    let tar_gz = std::fs::File::open(&temp_file_path)?;
    let decoder = GzDecoder::new(&tar_gz);
    let mut archive = Archive::new(decoder);
    archive.unpack(cache_path.join(dest_path))?;
    println!("[DOWNLOAD::INFO] Remove downloaded temp file");
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
        return Err(anyhow::anyhow!("[RUN::ERROR] Csound exited with non-zero status: {}", status));
    }

    Ok(())
}

pub fn check_risset() -> Result<()> {
    // check if risset is installed
    if process::Command::new("risset").output().is_err() {
        println!("[RISSET::WARNING] risset not found. Installing...");
        match env::consts::OS {
            "linux" | "macos" => {
                println!("[RISSET::INFO] Install uv");
                process::Command::new("curl")
                    .args(["-LsSf", "https://astral.sh/uv/install.sh", "|", "sh"])
                    .status()?;
            },
            "windows" => {
                println!("[RISSET::INFO] Install uv");
                process::Command::new("powershell")
                    .args(["-ExecutionPolicy", "ByPass"])
                    .args(["-c", "irm https://astral.sh/uv/install.ps1", "|", "iex"])
                    .status()?;
            },
            _ => return Err(anyhow::anyhow!("[RISSET::ERROR] Unknown OS"))
        }

        println!("[RISSET::INFO] Install risset");
        process::Command::new("uv")
            .arg("tool")
            .args(["install", "risset"])
            .status()?;

        println!("[RISSET::INFO] Upgrade risset");
        process::Command::new("uv")
            .arg("tool")
            .args(["upgrade", "risset"])
            .status()?;
    }

    println!("[RISSET::INFO] risset has been installed");

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
        return Err(anyhow::anyhow!("[RISSET::ERROR] Plugins installation exited with non-zero status: {}", status));
    }

    Ok(())
}
