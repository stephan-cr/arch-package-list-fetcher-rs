#![warn(rust_2018_idioms)]
#![warn(clippy::pedantic)]

use colored::Colorize;
use http_req::request;
use regex::RegexSet;
use rss::Channel;
use std::error::Error;
use std::fs::File;
use std::io::{self, ErrorKind, Read};
use toml::Value;
use xdg::BaseDirectories;

#[derive(thiserror::Error, Debug)]
enum ParseError {
    #[error("no filter set found")]
    MissingFilterSet,
    #[error("unknown type {0}")]
    UnknownType(Value),
}

fn parse_filter_regexes(input: &str) -> Result<Vec<String>, Box<dyn Error>> {
    let value = input.parse::<Value>()?;
    let mut result = vec![];

    match value {
        toml::Value::Table(table) => match table.get("filter_set") {
            Some(Value::Array(array)) => {
                for elem in array.iter() {
                    match elem {
                        Value::String(string) => result.push(string.into()),
                        other => return Err(Box::new(ParseError::UnknownType(other.clone()))),
                    }
                }
            }
            Some(other) => return Err(Box::new(ParseError::UnknownType(other.clone()))),
            None => return Err(Box::new(ParseError::MissingFilterSet)),
        },
        other => return Err(Box::new(ParseError::UnknownType(other))),
    };

    Ok(result)
}

fn main() -> Result<(), Box<dyn Error>> {
    let result = || -> Result<(), Box<dyn Error>> {
        let xdg_dirs = BaseDirectories::new()?;
        let config_path = xdg_dirs
            .find_config_file("arch-package-list-fetcher.config")
            .ok_or_else(|| io::Error::new(ErrorKind::NotFound, "not found"))?;
        let mut input = String::new();
        File::open(&config_path).and_then(|mut f| f.read_to_string(&mut input))?;

        let filter_regexes = parse_filter_regexes(&input).unwrap_or_else(|e| {
            eprintln!("{}", e);
            std::process::exit(1);
        });
        let set = RegexSet::new(&filter_regexes)?;

        let mut content = Vec::new();
        let res = request::get("https://archlinux.org/feeds/packages/", &mut content)?;

        if res.status_code().is_success() {
            let channel = Channel::read_from(&content[..])?;
            for item in channel.into_items() {
                let title = item.title().map_or("unknown title", |title| title);
                if set.is_match(title) {
                    continue;
                }

                if item
                    .categories()
                    .iter()
                    .any(|cat| cat.name().contains("Testing"))
                {
                    continue;
                }

                let v: Vec<&str> = title.split(' ').collect();
                let (package_name, package_version) = (&v[0], &v[1]);
                println!("{} {}", package_name.green(), package_version.red());
            }
        }

        Ok(())
    }();

    if let Err(err) = result {
        eprintln!("{}", err);
        Err(err)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::io;
    use toml::Value;

    #[test]
    fn test_config() -> Result<(), io::Error> {
        let toml_str = r#"
            filter_set = [ "^haskell-", "^php\\d?-?" ]
        "#;

        let value = toml_str.parse::<Value>()?;
        match value {
            Value::Table(ref table) => match table.get("filter_set") {
                Some(Value::Array(ref array)) => {
                    for elem in array {
                        match elem {
                            Value::String(string) => eprintln!("string {}", string),
                            _ => panic!("unexpected type"),
                        }
                    }
                }
                _ => panic!("unexpected type"),
            },
            _ => panic!("unexpected type"),
        }
        eprintln!("{:?}", value);

        Ok(())
    }
}
