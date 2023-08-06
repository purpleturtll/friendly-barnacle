use std::error::Error;
mod package;
use env_logger;
use package::Package;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        return Err("Invalid number of arguments".into());
    }

    let mut package = match Package::from_url(&args[1]).await {
        Ok(package) => package,
        Err(e) => {
            println!("Error: {}", e);
            return Err(e);
        }
    };
    package.get_license().await?;
    package.get_dependencies().await?;
    package.print_tree();

    Ok(())
}
