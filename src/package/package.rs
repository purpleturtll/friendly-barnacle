use log::debug;
use std::{
    error::Error,
    fmt::{self, Formatter},
};

pub struct Package {
    name: String,
    owner: String,
    version: String,
    license: String,
    dependencies: Vec<Package>,
}

// public methods
impl Package {
    pub fn new(name: String, owner: String, version: String) -> Package {
        Package {
            name,
            owner,
            version,
            license: String::new(),
            dependencies: Vec::new(),
        }
    }

    pub fn from_url(url: &str) -> Result<Package, Box<dyn Error>> {
        let (name, owner, version) = Package::get_identifiers(url).unwrap();
        Ok(Package::new(name, owner, version))
    }

    pub async fn get_license(&mut self) -> Result<(), Box<dyn Error>> {
        let content = octocrab::instance()
            .repos(self.owner.clone(), self.name.clone())
            .license()
            .await?;
        self.license = content.license.unwrap().name;
        Ok(())
    }

    // Get dependencies from go.mod file
    #[async_recursion::async_recursion]
    pub async fn get_dependencies(&mut self) -> Result<(), Box<dyn Error>> {
        let response = octocrab::instance()
            .repos(self.owner.clone(), self.name.clone())
            .get_content()
            .path("go.mod")
            .send()
            .await?;
        // TODO: Support for multiple go.mod files
        if response.items.len() != 1 {
            return Err("Invalid number of go.mod files".into());
        }
        let content = response.clone().take_items()[0].decoded_content().unwrap();
        let lines: Vec<&str> = content.split("\n").collect();

        // There are 4 cases for dependencies:
        // 1. require github.com/owner/repo v1.0.0
        // 2. require github.com/owner/repo v1.0.0 // indirect
        // 3. require (
        //      ...
        //    )
        // 4. no dependencies

        // Case 1 and 2
        let mut i = 0;
        while i < lines.len() {
            if lines.get(i).unwrap().contains("require ") && !lines.get(i).unwrap().contains("(") {
                let line = lines.get(i).unwrap().trim().trim_end();
                let tokens: Vec<&str> = line.split(" ").collect();
                debug!("Tokens: {:?}", tokens);
                let url = tokens.get(1).unwrap();
                let url = format!("{}@{}", url, tokens.get(2).unwrap());
                let package = Package::from_url(url.as_str())?;
                self.dependencies.push(package);
            }
            i += 1;
        }

        // Case 3
        let mut i = 0;
        while i < lines.len() {
            if lines.get(i).unwrap().contains("require (") {
                i += 1;
                while !lines.get(i).unwrap().contains(")") {
                    let line = lines.get(i).unwrap().trim().trim_end();
                    let tokens: Vec<&str> = line.split(" ").collect();
                    debug!("Tokens: {:?}", tokens);
                    let url = tokens.get(0).unwrap();
                    let url = format!("{}@{}", url, tokens.get(1).unwrap());
                    let package = Package::from_url(url.as_str())?;
                    self.dependencies.push(package);
                    i += 1;
                }
            }
            i += 1;
        }

        for dependency in &mut self.dependencies {
            dependency.get_license().await?;
            debug!("Found dependency for {}: {:?}", self.name, dependency.name);
            dependency.get_dependencies().await?;
        }

        Ok(())
    }

    // Print tree of dependencies with indentation.
    pub fn print_tree(&self) {
        self.print_tree_recursive(0);
    }
}

// private methods
impl Package {
    fn platform(url: &str) -> Result<&str, Box<dyn Error>> {
        let tokens: Vec<&str> = url.split("/").collect();
        Ok(tokens.get(0).unwrap())
    }

    // Get name, owner, version from url
    fn get_identifiers(url: &str) -> Result<(String, String, String), Box<dyn Error>> {
        let platform = Package::platform(url)?;
        match platform {
            "github.com" => Package::get_github_identifiers(url),
            _ => Err(format!("Invalid platform: {}", platform).into()),
        }
    }

    fn get_github_identifiers(url: &str) -> Result<(String, String, String), Box<dyn Error>> {
        let tokens: Vec<&str> = url.split("/").collect();

        if tokens.len() != 3 {
            return Err("Invalid argument format".into());
        }
        let repo_name_version: Vec<&str> = tokens.get(2).unwrap().split("@").collect();
        if repo_name_version.len() != 2 {
            return Err("Invalid argument format (repo@version)".into());
        }

        let repo_name: &str = repo_name_version.get(0).unwrap();
        let repo_version: &str = repo_name_version.get(1).unwrap();
        let owner: &str = tokens.get(1).unwrap();
        Ok((
            String::from(repo_name),
            String::from(owner),
            String::from(repo_version),
        ))
    }

    // Recursive function to print tree of dependencies
    fn print_tree_recursive(&self, indent: usize) {
        let mut indent_str = String::new();
        for _ in 0..indent {
            indent_str.push_str("  ");
        }
        println!("{}{:?}", indent_str, self);
        for dependency in &self.dependencies {
            dependency.print_tree_recursive(indent + 1);
        }
    }
}

impl fmt::Debug for Package {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        // TODO: Print dependencies
        write!(
            f,
            "Package {{ name: {}, owner: {}, version: {}, license: {} }}",
            self.name, self.owner, self.version, self.license
        )
    }
}
