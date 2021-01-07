mod db;


/////////////////////////////////////////////////////////////////////////
/// STRUCTS

struct Cpu {
    name: String,
    single_core_score: Vec<u32>,
    multi_core_score: Vec<u32>,
    is_in_db: Exist,
    is_on_internet: Exist,
    pages: Vec<String>,
    number_of_pages: u32,
}

impl Default for Cpu {
    fn default() -> Self {
        Self {
            name: String::from(""),
            single_core_score: Vec::new(),
            multi_core_score: Vec::new(),
            is_in_db: Exist::Unknown,
            is_on_internet: Exist::Unknown,
            pages: Vec::new(),
            number_of_pages: 0,
        }
     }
}


#[derive(PartialEq)]
enum Exist {
    True,
    False,
    Unknown,
}


/////////////////////////////////////////////////////////////////////////
/// FUNCTIONS

fn to_url(cpu_name: &str, page: u32) -> String {
    format!("{}{}{}{}", 
        "https://browser.geekbench.com/v5/cpu/search?utf8=%E2%9C%93&page=", 
        page, 
        "&q=", 
        cpu_name.to_string().replace(" ", "+")
        )
}


async fn download_page(url: String) -> Result< String, Box<dyn std::error::Error>> {
    Ok(reqwest::get(&url).await?.text().await?)
}


async fn download_pages(urls: &Vec<String>) -> Result<Vec<String>, Box<dyn std::error::Error>> {

    let client = reqwest::Client::new();
    let mut pages = Vec::new();
    
    // DEBUG
    println!();

    for (i, url) in urls.iter().enumerate() {
        // DEBUG
        println!("{}Downloading the page {}/{} : {}", term_cursor::Up(1), i+2, urls.len()+1, urls[i]);
        
        pages.push(client.get(url).send().await?.text().await?);
    }
    
    Ok(pages)
}


async fn check_if_cpus_exists(cpus: &mut Vec<Cpu>) -> Result<(), Box<dyn std::error::Error>> {

    // Checking cpus
    // DEBUG
    println!("Checking if cpus exists...");
    println!();

    let number_of_cpus = cpus.len();
    for (i, cpu) in cpus.iter_mut().enumerate() {
        
        // DEBUG
        println!("{}Checking {}/{} : {}", term_cursor::Up(1), i+1, number_of_cpus, cpu.name);
        
        if cpu.is_in_db == Exist::True {
            continue;
        }
        
        let link: String = to_url(&cpu.name, 1);
        let page: String = download_page(link).await.unwrap();
        cpu.pages.push(page);
        
        // Find number of pages to download
        let document = scraper::Html::parse_document(&cpu.pages.first().unwrap());
        let selector = scraper::Selector::parse(".page-item:nth-last-child(2) > a").unwrap();

        cpu.number_of_pages = 
            match document.select(&selector)
            .nth(0)
             {
                Some(v) => {
                    cpu.is_on_internet = Exist::True;
                    v
                },
                None => {
                    cpu.is_on_internet = Exist::False;
                    cpu.number_of_pages = 0;
                    continue;
                },
            }
            .inner_html()
            .parse()
            .unwrap();
    }

    return Ok(())
}


/////////////////////////////////////////////////////////////////////////
/// MAIN

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    
    // Take care of cli arguments
    let args_matches = clap::App::new("Geekbench Distribution")
        .version("0.1.0")
        .author("Krzysztof Kwiatkowski <kthateyo@gmail.com>")
        .about("Downloads data about scores of specified cpus and plot histograms about them.\n\nProgram creates db.sqlite file in working directory to store cache of downloaded scores. In case you want to download updated scores, you can delete this file.")
        .arg(clap::Arg::new("cpu_names")
            .about("The list of cpus to compare. If they contain spaces, you can enclose them in quotes example: \"Intel i7 3770\" \"Intel i7 2700K\"")
            .takes_value(true)
            .multiple(true)
            .required(true)
        ).get_matches();


    // Create vec of Cpus
    let mut cpus: Vec<Cpu> = Vec::new();


    // Prepare items and fill cpus tables
    for arg in args_matches.values_of("cpu_names").unwrap() {
        let cpu = Cpu {
            name: String::from(arg),
            is_in_db: if db::is_table_exists(&String::from(arg)).unwrap() 
                { Exist::True } else { Exist::False },
            ..Cpu::default()
        };
        cpus.push(cpu);
    }

    // Check if cpus exists on the site

    check_if_cpus_exists(&mut cpus).await?;

    let mut not_found = false;
    for cpu in &cpus {
        if cpu.is_on_internet == Exist::False {
            not_found = true;
            println!("[ERROR] Cpu named: \"{}\" not found on the geekbench website", cpu.name);
        }
    }
    if not_found {
        return Err("Some cpus were not found".into())
    }


    let number_of_cpus = cpus.len();

    // For each CPU not in db get data
    let mut index: usize = 0;
    for cpu in &mut cpus {

        // DEBUG
        println!("CPU {}/{} : {}", index+1, number_of_cpus, cpu.name);
        
        if cpu.is_in_db == Exist::False {

            // Download all pages
            let urls = (2..=cpu.number_of_pages)
            .map(|i|
                to_url(&cpu.name, i)
            ).collect();
            
            cpu.pages.append(&mut download_pages(&urls).await?);

            // Parse pages
            // DEBUG
            println!("Parsing HTML pages...");

            for i in 0..cpu.pages.len() {
                // DEBUG
                println!("{}Parsing the page {}/{}", term_cursor::Up(1), i+1, cpu.pages.len());
                
                let document = scraper::Html::parse_document(&cpu.pages[i]);
                
                // Find single core scores
                let mut selector = scraper::Selector::parse("div.list-col-inner > div.row > div.col-6:nth-child(4) > span.list-col-text-score").unwrap();
                for element in document.select(&selector) {
                    let score: u32 = element.inner_html().trim().to_string().parse()?;
                    cpu.single_core_score.push(score);
                }
                
                // Find multi core scores
                selector = scraper::Selector::parse("div.list-col-inner > div.row > div.col-6:nth-child(5) > span.list-col-text-score").unwrap();
                for element in document.select(&selector) {
                    let score: u32 = element.inner_html().trim().to_string().parse()?;
                    cpu.multi_core_score.push(score);
                }
            }
            
            // Prepare Values for saving to sqlite
            // DEBUG
            println!("Saving scores to db.sqlite...");
            
            let mut values: Vec<[u32; 2]> = Vec::with_capacity(cpu.single_core_score.len());
            
            for i in 0..cpu.single_core_score.len() {
                values.push([cpu.single_core_score[i], cpu.multi_core_score[i]]);
            }
            
            // Save data to db.sqlite
            db::create_table(&cpu.name)?;
            db::insert_rows(&cpu.name, values)?;
            
        } else {

            // Get data from sqlite
            // DEBUG
            println!("Getting data from db.sqlite...");
            let mut table = db::get_table(&cpu.name)?;
            
            cpu.single_core_score.append(&mut table[0]);
            cpu.multi_core_score.append(&mut table[1]);
        }

        index += 1;
    }
    
    // Draw plot

    let mut plot_single = plotly::Plot::new();
    let mut plot_multi = plotly::Plot::new();

    let layout = plotly::Layout::new()
        .title(plotly::common::Title::new("Single core score comparison"))
        .x_axis(plotly::layout::Axis::new().title(plotly::common::Title::new("Score")))
        .y_axis(plotly::layout::Axis::new().title(plotly::common::Title::new("Probabilistic density")))
        .bar_mode(plotly::layout::BarMode::Overlay)
        .bar_gap(0.05)
        .bar_group_gap(0.2);

    plot_single.set_layout(layout);

    let layout = plotly::Layout::new()
        .title(plotly::common::Title::new("Multi core score comparison"))
        .x_axis(plotly::layout::Axis::new().title(plotly::common::Title::new("Score")))
        .y_axis(plotly::layout::Axis::new().title(plotly::common::Title::new("Probabilistic density")))
        .bar_mode(plotly::layout::BarMode::Overlay)
        .bar_gap(0.05)
        .bar_group_gap(0.2);

    plot_multi.set_layout(layout);


    // Define traces

    for cpu in cpus {

        let trace_single = plotly::Histogram::new(cpu.single_core_score.clone())
            .hist_norm(plotly::histogram::HistNorm::ProbabilityDensity)
            .name(&cpu.name)
            .auto_bin_x(true)
            .marker(
                plotly::common::Marker::new()
                    .line(plotly::common::Line::new()
                    .width(0.5)),
            )
            .opacity(0.5)
            .x_bins(plotly::histogram::Bins::new(0.5, 2.8, 0.06));

        let trace_multi = plotly::Histogram::new(cpu.multi_core_score.clone())
            .hist_norm(plotly::histogram::HistNorm::ProbabilityDensity)
            .name(&cpu.name)
            .auto_bin_x(true)
            .marker(
                plotly::common::Marker::new()
                    .line(plotly::common::Line::new()
                    .width(0.5)),
            )
            .opacity(0.5)
            .x_bins(plotly::histogram::Bins::new(0.5, 2.8, 0.06));
        
        plot_single.add_trace(trace_single);
        plot_multi.add_trace(trace_multi);
    }


    plot_single.show();
    plot_multi.show();

    Ok(())
}
