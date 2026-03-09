use clap::{ Parser, Subcommand };

#[derive(Parser)]
#[command(name = "cspm")]
#[command(about = "A modern package manager for Csound")]
pub struct CsCli {

    /// Use global environment
    #[arg(short = 'g', long = "global", global = true)]
    pub global: bool,

    /// Force removal of dependencies
    #[arg(short = 'f', long = "force", global = true)]
    pub force: bool,

    #[command(subcommand)]
    pub command: CsCommands,
}

#[derive(Subcommand, Debug)]
pub enum CsCommands {
    /// Create a new Csound project
    #[command(short_flag = 'i', long_flag = "init")]
    Init {

        /// Project name
        #[arg(short, long)]
        name: String,

        /// Create csound module
        #[arg(short = 'm', long = "module")]
        module_flag: bool,

        /// Create csound project
        #[arg(short = 'p', long = "project")]
        project_flag: bool

    },

    /// Add dependencies to the project
    #[command(short_flag = 'a', long_flag = "add")]
    Add {
        module: Vec<String>, // use @major.minor.patch to specify the version
    },

    /// Reinstall dependencies to the project
    Reinstall {
        module: Vec<String>, // use @major.minor.patch to specify the version
    },

    /// Remove dependencies from the project
    #[command(short_flag = 'r', long_flag = "remove")]
    Remove {
        module: Vec<String>,
    },

    /// Update the project's dependencies
    #[command(short_flag = 'u', long_flag = "update")]
    Update {
        module: Option<Vec<String>>,
    },

    /// Manage cspm cache
    #[command(short_flag = 'c', long_flag = "cache")]
    Cache {

        /// Clean cache
        #[arg(long)]
        clean: bool,

        /// Remove obsolete modules
        #[arg(long)]
        prune: bool,

        /// List entire cache
        #[arg(long)]
        list: bool
    },

    /// Check the project's environment status
    Sync,

    /// Build project from manifest or lock file
    #[command(short_flag = 'b', long_flag = "build")]
    Build {
        #[arg(long = "from-lock")]
        from_lock: bool
    },

    /// Publish Csound module
    #[command(long_flag = "publish")]
    Publish,

    /// Run Csound project
    #[command(long_flag = "run")]
    Run {
        /// Specify Csound build script options
        #[arg(long = "csoptions", num_args = 1..)]
        csoptions: Option<Vec<String>>
    },

    /// Display module info
    Search {
        /// Specify the module you wanto info about
        module: String
    },

    /// Display cspm version
    #[command(short_flag = 'v', long_flag = "version")]
    Version
}
