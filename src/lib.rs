use std::{fs::File, io::Read, path::Path};

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Config {
    name: String,
    version: String,
}



/// Reads a YAML file and deserializes it into the specified type
pub fn read_yaml_file<T, P>(path: P) -> Result<T, Box<dyn std::error::Error>>
where
    T: for<'de> Deserialize<'de>,
    P: AsRef<Path>,
{
    // Open the file
    let mut file = File::open(path)?;
    
    // Read the file content
    let mut content = String::new();
    file.read_to_string(&mut content)?;
    
    // Parse YAML content
    let parsed: T = serde_yaml::from_str(&content)?;
    
    Ok(parsed)
}
