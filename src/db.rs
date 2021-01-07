extern crate rusqlite;

////////////////////////////////////////////////////////
/// VARIABLES

const DB_PATH: &str = "db.sqlite";


////////////////////////////////////////////////////////
/// PRIVATE FUNCTIONS

fn prepare_table_name(name: &str) -> String {
    return name.to_lowercase().replace(' ', "_").replace('+', "_").replace('-', "_")
}


////////////////////////////////////////////////////////
/// PUBLIC FUNCTIONS

pub fn is_table_exists(name: &String) -> rusqlite::Result<bool>  {

    let name: String = prepare_table_name(&name);
    let conn = rusqlite::Connection::open(DB_PATH)?;

    let result = conn.query_row(
        format!("SELECT COUNT(name) FROM sqlite_master WHERE type='table' AND name='{}'", name).as_str(), 
        rusqlite::NO_PARAMS, |row| row.get(0)
    );



    return result
}


pub fn create_table(name: &str) -> rusqlite::Result<()>  {

    let name: &str = &prepare_table_name(name);
    let conn = rusqlite::Connection::open(DB_PATH)?;
    
    match conn.execute(format!(
        "CREATE TABLE IF NOT EXISTS {} 
        (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            single_core_score UNSIGNED INTEGER NOT NULL,
            multi_core_score UNSIGNED INTEGER NOT NULL
        )", name).as_str(), rusqlite::params![]
    ) {
        Ok(_updated) => {
            // DEBUG
            // println!("Created table {}", name)
        }
        Err(err) => {
            // DEBUG
            println!("update failed: {}", err)
        }
    }

    Ok(())
}


pub fn insert_rows(table: &str, rows: Vec<[u32; 2]>) -> rusqlite::Result<()>  {

    let name: &str = &prepare_table_name(table);
    let conn = rusqlite::Connection::open(DB_PATH)?;
    
    let mut values: String = String::new();
    values.push_str(format!("({}, {})", rows[0][0], rows[0][1]).as_str());
    
    for i in 1..rows.len() {
        values.push_str(format!(", ({}, {})", rows[i][0], rows[i][1]).as_str());
    }


    match conn.execute(format!("INSERT INTO {} (single_core_score, multi_core_score) VALUES {}", name, values).as_str(), rusqlite::params![]) 
    {
        Ok(_updated) => {
            // DEBUG
            //println!("Inserted rows into {}, values: {}", name, values);
        }
        Err(err) => {
            // DEBUG
            println!("Insert failed: {}", err);
        }
    }

    Ok(())
}


pub fn get_table(name: &str) -> rusqlite::Result<[Vec<u32>;2]>  {

    let name: &str = &prepare_table_name(name);
    let conn = rusqlite::Connection::open(DB_PATH)?;


    let mut statement = conn.prepare(
        format!("SELECT single_core_score, multi_core_score FROM {}", name).as_str()
    )?;

    let rows = statement.query_map(rusqlite::NO_PARAMS, |row| {
        Ok([
            row.get(0)?,
            row.get(1)?,
        ])
    })?;

    let mut single_core_scores: Vec<u32> = Vec::new();
    let mut multi_core_scores: Vec<u32> = Vec::new();

    for row in rows {
        match row {
            Ok(v) => {
                single_core_scores.push(v[0]);
                multi_core_scores.push(v[1]);
            }
            Err(e) => {
                println!("Error: {}", e);
            }
        }
    }

    return Ok([single_core_scores, multi_core_scores])
}

