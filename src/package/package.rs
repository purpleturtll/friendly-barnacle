use log::debug;
use reqwest::redirect::Policy;
use std::{
    error::Error,
    fmt::{self, format, Formatter},
};

pub struct Package {
    name: String,
    owner: String,
    version: String,
    license: String,
    source: String,
    dependencies: Vec<Package>,
}

// public methods
impl Package {
    pub fn new(name: String, owner: String, version: String, source: String) -> Package {
        Package {
            name,
            owner,
            version,
            source: source,
            license: String::new(),
            dependencies: Vec::new(),
        }
    }

    pub async fn from_url(url: &str) -> Result<Package, Box<dyn Error>> {
        let (name, owner, version, source) = Package::get_identifiers(url).await?;
        Ok(Package::new(name, owner, version, source))
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
                let package = Package::from_url(url.as_str()).await?;
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
                    let package = Package::from_url(url.as_str()).await?;
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
    async fn get_identifiers(
        url: &str,
    ) -> Result<(String, String, String, String), Box<dyn Error>> {
        let platform = Package::platform(url)?;
        match platform {
            "github.com" => Package::get_github_identifiers(url),
            "golang.org" => Package::get_golang_identifiers(url).await,
            _ => Err(format!("Invalid platform: {}", platform).into()),
        }
    }

    // To get repo we need to make request GET golang.org/x/sys, and extract actual repo from <meta name=go-import ...>
    async fn get_golang_identifiers(
        url: &str,
    ) -> Result<(String, String, String, String), Box<dyn Error>> {
        debug!("Getting identifiers for {}", url);
        // Split by url@version
        let tokens: Vec<&str> = url.split("@").collect();
        let url = tokens.get(0).unwrap();
        let version = tokens.get(1).unwrap();
        let client = reqwest::Client::builder()
            .redirect(Policy::none())
            .build()?;
        let response = client.get(format!("https://{}", url)).send().await?;
        let content = response.text().await?;
        debug!("Content: {}", content);
        let lines: Vec<&str> = content.split("\n").collect();
        let mut repo = String::new();
        for line in lines {
            if line.contains("meta name=\"go-import\"") {
                let tokens: Vec<&str> = line.split(" ").collect();
                debug!("Tokens: {:?}", tokens);
                repo = tokens.get(tokens.len() - 1).unwrap().to_string();
                repo = repo.replace("\"", "");
                repo = repo.replace(">", "");
                break;
            }
        }
        let tokens: Vec<&str> = repo.split("/").collect();
        debug!("Tokens: {:?}", tokens);
        let owner = "".to_string();
        let name = tokens.get(1).unwrap().to_string();
        Ok((name, owner, version.to_string(), repo))
    }

    fn get_github_identifiers(
        url: &str,
    ) -> Result<(String, String, String, String), Box<dyn Error>> {
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
            String::from(format!("github.com/{}/{}", owner, repo_name)),
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
