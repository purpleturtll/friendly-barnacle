#[allow(dead_code)]
#[allow(unused_variables)]
use std::{error::Error, fmt::{Formatter, self}};
use octocrab::{self, models::repos::Content};
use base64;

struct Package {
    name : String,
    owner : String,
    version : String,
    license : String,
    dependencies : Vec<Package>,
}

impl fmt::Debug for Package {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        // Print package in table format without dependencies
        write!(f, "-----\nPackage: {}\nVersion: {}\nLicense: {}\n", self.name, self.version, self.license)
    }
}

impl Package {
    // Contructor
    fn new(name : String, owner : String, version : String) -> Package {
        Package {
            name,
            owner,
            version,
            license : String::new(),
            dependencies : Vec::new(),
        }
    }

    // Dependency names are in the form of github.com/owner/repo
    pub async fn get_license(&mut self) -> Result<Content, Box<dyn Error>> {
        let tokens : Vec<&str> = self.name.split("/").collect();
        println!("{:?}", tokens);
        let owner = tokens.get(1).unwrap();
        let repo = tokens.get(2).unwrap();
        let license = get_license(owner, repo).await?;
        self.license = license.clone().license.unwrap().name;
        Ok(license)
    }

    // Get dependencies from go.mod file
    pub async fn get_dependencies(&mut self) -> Result<(), Box<dyn Error>> {
        let go_mod_deps = get_go_mod_deps(owner, repo).await;
        for dep in go_mod_deps {
            self.dependencies.push(dep);
        }
        Ok(())
    }

}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Get name of repository to analize command line argument
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        return Err("Invalid number of arguments".into());
    }
    // Check if argument is in the form of github.com/owner/repo@version
    check_arg_format(args.get(1).unwrap())?;
    let repo_url = args.get(1).unwrap();
    let repo_components : Vec<&str> = repo_url.split("/").collect();
    let repo_name_version: Vec<&str> = repo_components.get(2).unwrap().split("@").collect();
    let repo_name: &str = repo_name_version.get(0).unwrap();
    let repo_version: &str = repo_name_version.get(1).unwrap();
    let owner: &str = repo_components.get(1).unwrap();


    // Print info about root package
    println!("-----\nPackage: {}\nVersion: {}\n", repo_name, repo_version);

    let mut package = Package::new(String::from(repo_name), String::from(owner), String::from(repo_version));
    package.get_dependencies().await?;
    package.get_license().await?;

    println!("{:?}", package);

    Ok(())
}

fn check_arg_format(repo_url: &str) -> Result<(), Box<dyn Error>> {
    let repo_components : Vec<&str> = repo_url.split("/").collect();
    if repo_components.len() != 3 {
        return Err("Invalid repository format".into());
    }
    let repo_name_version: Vec<&str> = repo_components.get(2).unwrap().split("@").collect();
    if repo_name_version.len() != 2 {
        return Err("Invalid repository format".into());
    }
    Ok(())
}

// Get go.mod file from repository
async fn get_go_mod(owner: &str, repo: &str) -> Result<Content, Box<dyn Error>> {
    let result = octocrab::instance().repos(owner, repo).get_content().path("go.mod").send().await;
    let content_list = match result {
        Ok(go_mod) => go_mod,
        Err(e) => return Err(Box::new(e)),
    };
    if content_list.items.len() != 1 {
        return Err("go.mod not found".into());
    };
    let go_mod = content_list.items[0].clone();
    Ok(go_mod)
}

// Get repository license from GitHub
async fn get_license(owner: &str, repo: &str) -> Result<Content, octocrab::Error> {
    let license = octocrab::instance().repos(owner, repo).license().await;
    license
}

// Get default branch of repository
async fn get_default_branch(owner: &str, repo: &str) -> Result<String, octocrab::Error> {
    let repo = octocrab::instance().repos(owner, repo).get().await?;
    Ok(repo.default_branch.unwrap())
}