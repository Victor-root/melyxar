//! Playing the pages somebody browsing would ask for, and timing them.
//!
//! The budgets this server is held to are written down in the reactivity
//! document, in milliseconds, at the ninety fifth percentile, on a library of
//! a hundred thousand. Written down and never checked, they are opinions. This
//! is what checks them: it asks the running server for the same pages a person
//! clicking through would, several hundred times each, and puts what it
//! measured next to what was promised.
//!
//! Against the server over the network rather than against the database
//! directly, because what a page costs is the whole of it: reading, shaping
//! the answer, and writing it out. Measured from the same machine, so what is
//! added on top of the server's own time is a loopback and nothing else.
//!
//! It also asks the server what *it* thinks each answer took, out of the
//! header every answer carries, so the two numbers can be put side by side. A
//! wide gap between them is a gap that is not the server's, and knowing that
//! is what stops an afternoon being spent on the wrong half.

use std::time::{Duration, Instant};

use anyhow::Context;
use reqwest::Url;

/// What a run was asked for.
pub struct Asked {
    /// Where the server is answering.
    pub address: String,
    /// How many times each page is asked for, once the warm up is done.
    pub rounds: u32,
    /// How many are asked for at the same time.
    ///
    /// One person browsing is one at a time, which is the number the budgets
    /// are written about. More than one says what happens when the household
    /// is all watching at once.
    pub at_once: u32,
}

/// What a page is allowed to cost, and what a run measured it at.
struct Scenario {
    /// What somebody is doing, said the way they would say it.
    what: &'static str,
    url: Url,
    /// The budget from the reactivity document, in milliseconds. None where
    /// the document names no figure, which is a page worth watching rather
    /// than a page worth failing over.
    budget_ms: Option<f64>,
}

/// What one scenario measured.
struct Measured {
    what: &'static str,
    budget_ms: Option<f64>,
    /// Every answer, in milliseconds, as the client saw it.
    client: Vec<f64>,
    /// What the server said it took, out of the header on each answer.
    server: Vec<f64>,
    failures: u32,
}

impl Measured {
    /// The value below which this share of the answers came in.
    fn percentile(sorted: &[f64], share: f64) -> f64 {
        if sorted.is_empty() {
            return 0.0;
        }
        let rank = (share * sorted.len() as f64).ceil() as usize;
        sorted[rank.clamp(1, sorted.len()) - 1]
    }

    fn held(&self) -> bool {
        match self.budget_ms {
            Some(budget) => self.failures == 0 && Self::percentile(&self.client, 0.95) <= budget,
            None => self.failures == 0,
        }
    }
}

/// Runs every scenario and answers whether every budget was held.
pub async fn run(asked: Asked) -> anyhow::Result<bool> {
    anyhow::ensure!(asked.rounds > 0, "a page asked for no times measures nothing");
    anyhow::ensure!(asked.at_once > 0, "somebody has to be asking");

    // Never through a proxy: the server is on this machine, and a proxy in
    // between would be the thing being measured.
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(30))
        .build()
        .context("preparing the client")?;
    let base = Url::parse(&asked.address)
        .with_context(|| format!("reading the address {}", asked.address))?;

    let ground = look_around(&client, &base)
        .await
        .context("looking at what this server holds")?;
    println!(
        "measuring against '{}', {} work(s), {} round(s) each, {} at a time\n",
        ground.library_name, ground.works, asked.rounds, asked.at_once
    );

    let mut every = Vec::new();
    for scenario in scenarios(&base, &ground)? {
        every.push(measure(&client, &scenario, &asked).await);
    }

    report(&every);
    Ok(every.iter().all(Measured::held))
}

/// What this server holds, read once so the scenarios can point at real works.
struct Ground {
    library: String,
    library_name: String,
    works: i64,
    /// Works to open pages of, taken from several places in the ordering.
    ids: Vec<String>,
    /// A card to start a second page after.
    after: Option<String>,
    genre: Option<String>,
    decade: Option<i32>,
    /// The beginning of a real title, for the search.
    prefix: Option<String>,
}

async fn look_around(client: &reqwest::Client, base: &Url) -> anyhow::Result<Ground> {
    let libraries = fetch(client, base.join("api/v1/libraries")?).await?;
    // The largest library on this server, which is the invented one wherever
    // there is one: measuring against fifty films says nothing.
    let library = libraries
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("this server answered no list of libraries"))?
        .iter()
        .max_by_key(|library| library["works"].as_i64().unwrap_or_default())
        .ok_or_else(|| anyhow::anyhow!("this server holds no library to measure against"))?;
    let id = library["id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("a library with no identifier"))?
        .to_string();

    let page = fetch(client, grid(base, &id, &[("limit", "100")])?).await?;
    let cards = page["cards"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("a page with no cards"))?;
    anyhow::ensure!(
        !cards.is_empty(),
        "the library to measure against is empty; fill it first"
    );

    let filters = fetch(client, base.join(&format!("api/v1/libraries/{id}/filters"))?).await?;

    Ok(Ground {
        library: id,
        library_name: library["name"].as_str().unwrap_or("?").to_string(),
        works: library["works"].as_i64().unwrap_or_default(),
        // Spread through the page rather than the first few, so the pages
        // opened are not all neighbours in the ordering.
        ids: cards
            .iter()
            .step_by(7)
            .filter_map(|card| card["id"].as_str().map(str::to_string))
            .collect(),
        after: page["next"].as_str().map(str::to_string),
        genre: filters["genres"][0]["name"].as_str().map(str::to_string),
        decade: filters["decades"][0]["decade"].as_i64().map(|it| it as i32),
        prefix: cards[0]["title"]
            .as_str()
            .map(|title| title.chars().take(4).collect()),
    })
}

/// The pages a run asks for, each with what it is allowed to cost.
fn scenarios(base: &Url, ground: &Ground) -> anyhow::Result<Vec<Scenario>> {
    let library = ground.library.as_str();
    let mut scenarios = vec![
        Scenario {
            what: "grid, first page of 100",
            url: grid(base, library, &[("limit", "100")])?,
            budget_ms: Some(30.0),
        },
        Scenario {
            what: "grid, newest first",
            url: grid(
                base,
                library,
                &[("limit", "100"), ("order", "added_at"), ("descending", "true")],
            )?,
            budget_ms: Some(30.0),
        },
        Scenario {
            what: "grid, jump to a letter",
            url: grid(base, library, &[("limit", "100"), ("initial", "s")])?,
            budget_ms: Some(30.0),
        },
        Scenario {
            what: "the menus that narrow a grid",
            url: base.join(&format!("api/v1/libraries/{library}/filters"))?,
            budget_ms: None,
        },
        Scenario {
            what: "home page",
            url: base.join("api/v1/home")?,
            budget_ms: None,
        },
    ];

    if let Some(after) = &ground.after {
        scenarios.push(Scenario {
            what: "grid, the page after",
            url: grid(base, library, &[("limit", "100"), ("after", after)])?,
            budget_ms: Some(30.0),
        });
    }
    if let Some(genre) = &ground.genre {
        scenarios.push(Scenario {
            what: "grid, narrowed to one genre",
            url: grid(base, library, &[("limit", "100"), ("genre", genre)])?,
            budget_ms: Some(30.0),
        });
    }
    if let Some(decade) = ground.decade {
        scenarios.push(Scenario {
            what: "grid, narrowed to one decade",
            url: grid(
                base,
                library,
                &[("limit", "100"), ("decade", &decade.to_string())],
            )?,
            budget_ms: Some(30.0),
        });
    }
    if let Some(prefix) = &ground.prefix {
        scenarios.push(Scenario {
            what: "search on the start of a title",
            url: grid(base, library, &[("limit", "100"), ("search", prefix)])?,
            budget_ms: Some(30.0),
        });
    }
    for (rank, id) in ground.ids.iter().take(3).enumerate() {
        scenarios.push(Scenario {
            what: ["detail page", "detail page, another", "detail page, a third"][rank],
            url: base.join(&format!("api/v1/works/{id}"))?,
            budget_ms: Some(20.0),
        });
    }

    Ok(scenarios)
}

/// The address of one grid, with what narrows it.
fn grid(base: &Url, library: &str, narrowing: &[(&str, &str)]) -> anyhow::Result<Url> {
    let mut url = base.join("api/v1/works")?;
    {
        let mut pairs = url.query_pairs_mut();
        pairs.append_pair("library", library);
        for (name, value) in narrowing {
            pairs.append_pair(name, value);
        }
    }
    Ok(url)
}

async fn fetch(client: &reqwest::Client, url: Url) -> anyhow::Result<serde_json::Value> {
    let response = client
        .get(url.clone())
        .send()
        .await
        .with_context(|| format!("asking for {url}"))?;
    anyhow::ensure!(
        response.status().is_success(),
        "{url} answered {}",
        response.status()
    );
    response
        .json()
        .await
        .with_context(|| format!("reading the answer to {url}"))
}

/// Asks for one page over and over, and keeps what each one took.
async fn measure(client: &reqwest::Client, scenario: &Scenario, asked: &Asked) -> Measured {
    // A first answer pays for what every answer after it finds ready: a
    // statement prepared, a page of the file read off the disk. Counting it
    // would report the cost of the first visit of the day as the cost of every
    // visit.
    for _ in 0..asked.at_once.max(2) {
        let _ = one_at(client, &scenario.url).await;
    }

    let each = (asked.rounds / asked.at_once).max(1);
    let mut asking = Vec::new();
    for _ in 0..asked.at_once {
        let client = client.clone();
        let url = scenario.url.clone();
        asking.push(tokio::spawn(async move {
            let mut taken = Vec::with_capacity(each as usize);
            for _ in 0..each {
                taken.push(one_at(&client, &url).await);
            }
            taken
        }));
    }

    let mut measured = Measured {
        what: scenario.what,
        budget_ms: scenario.budget_ms,
        client: Vec::new(),
        server: Vec::new(),
        failures: 0,
    };
    for asker in asking {
        let taken = match asker.await {
            Ok(taken) => taken,
            Err(_) => {
                measured.failures += 1;
                continue;
            }
        };
        for answer in taken {
            match answer {
                Some((took, said)) => {
                    measured.client.push(took);
                    if let Some(said) = said {
                        measured.server.push(said);
                    }
                }
                None => measured.failures += 1,
            }
        }
    }
    measured.client.sort_by(f64::total_cmp);
    measured.server.sort_by(f64::total_cmp);
    measured
}

/// One answer: how long it took here, and what the server said it took.
///
/// The body is read to the end before the clock stops. An answer whose
/// headers have arrived is not an answer somebody can look at.
async fn one_at(client: &reqwest::Client, url: &Url) -> Option<(f64, Option<f64>)> {
    let started = Instant::now();
    let response = client.get(url.clone()).send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }
    let said = response
        .headers()
        .get("server-timing")
        .and_then(|value| value.to_str().ok())
        .and_then(server_said);
    response.bytes().await.ok()?;
    Some((started.elapsed().as_secs_f64() * 1_000.0, said))
}

/// How long the server said an answer took, out of its header.
fn server_said(header: &str) -> Option<f64> {
    header
        .split(',')
        .map(str::trim)
        .find(|metric| metric.starts_with("total;"))?
        .split("dur=")
        .nth(1)?
        .trim()
        .parse()
        .ok()
}

/// Puts what was measured next to what was promised.
fn report(every: &[Measured]) {
    println!(
        "{:<34} {:>7} {:>8} {:>8} {:>8} {:>8} {:>8}",
        "what somebody is doing", "asked", "p50", "p95", "p99", "server", "budget"
    );
    println!("{}", "-".repeat(86));

    for measured in every {
        let p95 = Measured::percentile(&measured.client, 0.95);
        println!(
            "{:<34} {:>7} {:>8.1} {:>8.1} {:>8.1} {:>8} {:>8} {}",
            measured.what,
            measured.client.len(),
            Measured::percentile(&measured.client, 0.50),
            p95,
            Measured::percentile(&measured.client, 0.99),
            match measured.server.is_empty() {
                true => "-".to_string(),
                false => format!("{:.1}", Measured::percentile(&measured.server, 0.95)),
            },
            match measured.budget_ms {
                Some(budget) => format!("{budget:.0}"),
                None => "-".to_string(),
            },
            verdict(measured)
        );
    }

    let missed: Vec<&Measured> = every.iter().filter(|one| !one.held()).collect();
    println!();
    match missed.is_empty() {
        true => println!("every budget held"),
        false => {
            println!("{} budget(s) missed:", missed.len());
            for one in missed {
                println!("  {}", one.what);
            }
        }
    }
    println!(
        "\np95 is the ninety fifth percentile: nineteen answers in twenty came in \
         under it.\n'server' is what the server itself said it took, out of the \
         header on each answer."
    );
}

fn verdict(measured: &Measured) -> String {
    if measured.failures > 0 {
        return format!("<- {} answer(s) failed", measured.failures);
    }
    let (Some(budget), p95) = (
        measured.budget_ms,
        Measured::percentile(&measured.client, 0.95),
    ) else {
        return String::new();
    };
    match p95 <= budget {
        true => String::new(),
        false => format!("<- over by {:.1} ms", p95 - budget),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_percentile_is_the_value_that_share_came_in_under() {
        let sorted: Vec<f64> = (1..=100).map(f64::from).collect();
        assert_eq!(Measured::percentile(&sorted, 0.50), 50.0);
        assert_eq!(Measured::percentile(&sorted, 0.95), 95.0);
        assert_eq!(Measured::percentile(&sorted, 0.99), 99.0);
        assert_eq!(Measured::percentile(&[], 0.95), 0.0);
        assert_eq!(Measured::percentile(&[7.0], 0.95), 7.0);
    }

    #[test]
    fn what_the_server_said_is_read_off_its_header() {
        assert_eq!(server_said("total;dur=18.4"), Some(18.4));
        assert_eq!(server_said("cache;dur=1, total;dur=2.5"), Some(2.5));
        assert_eq!(server_said("cache;dur=1"), None);
        assert_eq!(server_said(""), None);
        assert_eq!(server_said("total;dur=nonsense"), None);
    }

    #[test]
    fn a_budget_is_held_or_it_is_not() {
        let measured = |p95: f64, failures: u32| Measured {
            what: "grid",
            budget_ms: Some(30.0),
            client: vec![p95; 20],
            server: Vec::new(),
            failures,
        };
        assert!(measured(29.0, 0).held());
        assert!(measured(30.0, 0).held());
        assert!(!measured(30.1, 0).held());
        assert!(
            !measured(1.0, 1).held(),
            "an answer that never came is not a budget held"
        );
    }

    #[test]
    fn a_page_with_no_budget_is_watched_rather_than_failed() {
        let watched = Measured {
            what: "home page",
            budget_ms: None,
            client: vec![900.0],
            server: Vec::new(),
            failures: 0,
        };
        assert!(watched.held());
        assert!(verdict(&watched).is_empty());
    }

    #[test]
    fn a_grid_carries_what_narrows_it_however_it_is_written() {
        let base = Url::parse("http://127.0.0.1:2100").expect("an address");
        let url = grid(&base, "abc", &[("genre", "Comédie")]).expect("a grid");
        assert!(url.as_str().starts_with("http://127.0.0.1:2100/api/v1/works?"));
        assert!(
            url.as_str().contains("genre=Com%C3%A9die"),
            "an accent has to survive the address: {url}"
        );
    }
}
