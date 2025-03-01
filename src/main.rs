use loggen_rs::{read_yaml_file, Config};



fn main() {
    let path = "test_config.yaml";
    let result = read_yaml_file::<Config, &str>(path);
    match result {
        Ok(config) => {
            println!("{:?}", config);
        }
        Err(e) => {
            eprintln!("Error: {}", e);
        }
    }
}

