use std::env;
use std::io::ErrorKind;
use std::io::Error as SIError;
use std::io::{self, Write};
use serde::Deserialize;

use diesel::prelude::SqliteConnection;
use crate::db;


//result bool should be 0 if the main program needs to do its thing
//                      1 if the main program needs termination

pub async fn parse_cmdline() -> std::io::Result<bool> {
    let args: Vec<String> = env::args().collect();
    if args.len() > 1 {
        match args[1].as_str() {

            "text" => {
                if args.len() != 3 {
                    println!("Invalid syntax! Should be: ./racetyper text text_to_add");
                    return Err(SIError::new(ErrorKind::Other, ""));
                }
                let dbc = db::establish_connection();
                match db::create_typing_text(&dbc, args[2].clone()) {
                    Ok(sz) => {
                        if sz == 1 {
                            println!("Successfully added new text!");
                            return Ok(true);
                        } else {
                            println!("Text inertion faild, nrows: {}", sz);
                            return Err(SIError::new(ErrorKind::Other, ""));
                        }
                    },
                    Err(e) => {
                        println!("Failed adding new text!");
                        return Err(SIError::new(ErrorKind::Other, format!("Wrapped error: {:?}", e)));
                    },
                }
            },

            "csv" => {
                if args.len() != 3 {
                    println!("Invalid syntax! Should be: ./racetyper csv file.csv");
                    return Err(SIError::new(ErrorKind::Other, ""));
                }

                let dbc = db::establish_connection();
                return parse_csv_text(&dbc, &args[2]);
            },

            _ => {
                println!("No valid arguments found, skipping!");
                return Ok(false);
            },
        };
    }

    return Ok(false);
}

#[derive(Debug, Deserialize)]
struct QuoteRecord {
    quote: String,
    author: String,
    genre: String,
}


fn parse_csv_text(conn: &SqliteConnection, fname: &String) -> std::io::Result<bool> {
   let mut rdr = match csv::ReaderBuilder::new().delimiter(b';').from_path(fname) {
        Ok(r) => r,
        Err(e) => {
            return Err(SIError::new(ErrorKind::Other, format!("Wrapped error: {:?}", e)));
        },
    };


    let min_len = 170;
    let n_entries = 1000;

    let mut cnt = 0;
    let mut avg = 0;
    let mut quote_vec = Vec::<QuoteRecord>::with_capacity(n_entries);
    for result in rdr.deserialize() {
        let record: QuoteRecord = match result {
            Ok(r) => r,
            Err(e) => {
                eprintln!("Failed to parse record: {}", e);
                continue;
            }
        };
        if record.quote.len() > min_len {
            avg += record.quote.len();
            cnt += 1;
            quote_vec.push(record);
        }

        if cnt > n_entries {
            break;
        }
    }
    println!("Avg len: {}, num entries: {}.", ((avg as f64)/(cnt as f64)), cnt);

    print!("Import? (Y/N): ");
    io::stdout().flush().unwrap();
    let mut input = String::new();
    match io::stdin().read_line(&mut input) {
        Err(e) => {
            return Err(SIError::new(ErrorKind::Other, format!("Wrapped error: {:?}", e)));
        },
        Ok(_) => (),
    };

    if input == "y\n" || input == "Y\n" {
        for quote in &quote_vec {
            match db::create_typing_text(conn, quote.quote.clone()) {
                Ok(_) => (),
                Err(e) => {
                    println!("Text insertion failed: {:?}", e);
                }
            }
        }
    }

    return Ok(true);
}
