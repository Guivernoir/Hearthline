use std::env;
use std::error::Error;

mod architecture;
mod capacity_policy;
mod contracts;
mod runtime;
mod workflow;

fn main() -> Result<(), Box<dyn Error>> {
    match env::args().nth(1).as_deref() {
        Some("verify") => {
            architecture::verify()?;
            runtime::verify()?;
            capacity_policy::verify()?;
            workflow::verify()?;
            println!("Hearthline architecture and runtime policies passed.");
        }
        Some("contracts") => match env::args().nth(2).as_deref() {
            Some("--write") => contracts::write()?,
            Some("--check") => contracts::check()?,
            _ => return Err("usage: cargo xtask contracts --write|--check".into()),
        },
        _ => return Err("usage: cargo xtask verify|contracts --write|--check".into()),
    }
    Ok(())
}
