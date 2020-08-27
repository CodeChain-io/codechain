use clap::ArgMatches;

/// Generate large trie to test update speed
pub fn run_generate_data_command(_matches: &ArgMatches) -> Result<(), String> {
    println!("HI generate data");
    Ok(())
}
