use clap::ArgMatches;
use color_eyre::eyre::Result;

pub mod cmd_line;
mod logging;
mod module;
mod templating;
mod validation;
mod write;

pub use module::{Input, Module, ModuleType};

/// This struct stores options based on the command-line arguments,
/// and is passed to various functions across the program.
#[derive(Debug, Clone)]
pub struct Options {
    pub comments: bool,
    pub prefixes: bool,
    pub examples: bool,
    pub target_dir: String,
    pub verbosity: Verbosity,
}

impl Options {
    /// Set current options based on the command-line options
    pub fn new(args: &ArgMatches) -> Self {
        // Determine the configured verbosity level.
        // The clap configuration ensures that verbose and quiet
        // are mutually exclusive.
        let verbosity = if args.is_present("verbose") {
            Verbosity::Verbose
        } else if args.is_present("quiet") {
            Verbosity::Quiet
        } else {
            Verbosity::Default
        };

        Self {
            // Comments and prefixes are enabled (true) by default unless you disable them
            // on the command line. If the no-comments or no-prefixes option is passed
            // (occurences > 0), the feature is disabled, so the option is set to false.
            comments: !args.is_present("no-comments"),
            prefixes: !args.is_present("no-prefixes"),
            examples: !args.is_present("no-examples"),
            // Set the target directory as specified or fall back on the current directory
            target_dir: if let Some(dir) = args.value_of("target-dir") {
                String::from(dir)
            } else {
                String::from(".")
            },
            verbosity,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Verbosity {
    Verbose,
    Default,
    Quiet,
}

pub fn run(options: Options, cmdline_args: ArgMatches) -> Result<()> {
    // Initialize the logging system based on the set verbosity
    logging::initialize_logger(options.verbosity)?;

    log::debug!("Active options:\n{:#?}", &options);

    // Store all modules except for the populated assembly that will be created in this Vec
    let mut non_populated: Vec<Module> = Vec::new();

    // TODO: Maybe attach these strings to the ModuleType enum somehow
    // For each module type, see if it occurs on the command line and process it
    for module_type_str in ["assembly", "concept", "procedure", "reference", "snippet"] {
        // Check if the given module type occurs on the command line
        if let Some(titles_iterator) = cmdline_args.values_of(module_type_str) {
            let mut modules = process_module_type(titles_iterator, module_type_str, &options);

            // Move all the newly created modules into the common Vec
            non_populated.append(&mut modules);
        }
    }

    // Write all non-populated modules to the disk
    for module in &non_populated {
        module.write_file(&options)?;
    }

    // Treat the populated assembly module as a special case:
    // * There can be only one populated assembly
    // * It must be generated after the other modules so that it can use their include statements
    if let Some(title) = cmdline_args.value_of("include-in") {
        // Gather all include statements for the other modules
        let include_statements: Vec<String> = non_populated
            .into_iter()
            .map(|module| module.include_statement)
            .collect();

        // The include_statements should never be empty thanks to the required group in clap
        assert!(!include_statements.is_empty());

        // Generate the populated assembly module
        let populated: Module = Input::new(ModuleType::Assembly, title, &options)
            .include(include_statements)
            .into();

        populated.write_file(&options)?;
    }

    // Validate all file names specified on the command line
    if let Some(files_iterator) = cmdline_args.values_of("validate") {
        for file in files_iterator {
            validation::validate(file)?;
        }
    }

    Ok(())
}

/// Process all titles that have been specified on the command line and that belong to a single
/// module type.
fn process_module_type(
    titles: clap::Values,
    module_type_str: &str,
    options: &Options,
) -> Vec<Module> {
    let module_type = match module_type_str {
        "assembly" | "include-in" => ModuleType::Assembly,
        "concept" => ModuleType::Concept,
        "procedure" => ModuleType::Procedure,
        "reference" => ModuleType::Reference,
        "snippet" => ModuleType::Snippet,
        _ => unimplemented!(),
    };

    let modules_from_type = titles.map(|title| Module::new(module_type, title, options));

    modules_from_type.collect()
}
