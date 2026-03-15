use clap::{ Parser, Subcommand };

#[derive(Parser)]
#[command(name = "cspm")]
#[command(about = "A modern package manager for Csound")]
pub struct CsCli {

    #[command(subcommand)]
    pub command: CsCommands,
}

#[derive(Subcommand, Debug)]
pub enum CsCommands {
    /// Create a new Csound project
    Init {

        /// Install modules globally. Default = false
        #[arg(short, long)]
        global: bool,

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
    Add {
        module: Vec<String>, // use @major.minor.patch to specify the version

        /// force resolve dependencies
        #[arg(short = 'f', long = "force")]
        force: bool
    },

    /// Install modules globally (without manifest)
    Install {
        module: Vec<String>, // use @major.minor.patch to specify the version

        /// force resolve dependencies
        #[arg(short = 'f', long = "force")]
        force: bool
    },

    /// Uninstall modules globally (without manifest)
    Uninstall {
        module: Vec<String>, // use @major.minor.patch to specify the version

        /// force resolve dependencies
        #[arg(short = 'f', long = "force")]
        force: bool
    },

    /// Upgrade global modules (without manifest)
    Upgrade {
        module: Option<Vec<String>>,

        /// force resolve dependencies
        #[arg(short = 'f', long = "force")]
        force: bool
    },

    /// Reinstall dependencies to the project
    Reinstall {
        module: Vec<String>, // use @major.minor.patch to specify the version

        /// force resolve dependencies
        #[arg(short = 'f', long = "force")]
        force: bool
    },

    /// Remove dependencies from the project
    Remove {
        module: Vec<String>,

        /// force resolve dependencies
        #[arg(short = 'f', long = "force")]
        force: bool
    },

    /// Update the project dependencies
    Update {
        module: Option<Vec<String>>,

        /// force resolve dependencies
        #[arg(short = 'f', long = "force")]
        force: bool

    },

    /// Manage cspm cache
    Cache {

        /// Clean cache
        #[arg(long)]
        clean: bool,

        /// List entire cache folder
        #[arg(long)]
        list: bool
    },

    /// Check the project environment status
    Sync,

    /// Build project from manifest or lock file
    Build {

        /// Build using globally installed modules. Default = false
        #[arg(short = 'g', long = "global")]
        global: bool,

        /// Build using lockfile. Default = false (use manifest)
        #[arg(long = "from-lock")]
        from_lock: bool
    },

    /// Validate module structure and metadata before creating a Pull Request
    Publish,

    /// Run Csound project
    Run {
        /// Specify Csound build options
        #[arg(long = "csoptions", num_args = 0.., trailing_var_arg = true)]
        csoptions: Vec<String>
    },

    /// Check Cspm.toml file and fixes issues automatically
    Validate,

    /// Install plugins using risset
    Risset {
        /// Specify Risset options
        #[arg(num_args = 0.., trailing_var_arg = true)]
        rstoptions: Vec<String>
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
