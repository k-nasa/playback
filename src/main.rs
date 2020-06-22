use chrono::{DateTime, Duration, NaiveDateTime, Utc};
use clap::{App, AppSettings, Arg};
use reqwest::{Method, Url};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::convert::TryInto;
use std::str::FromStr;
use std::time;
use tokio::task;
use tokio::time::delay_for;

type PlaybackResult<T> = std::result::Result<T, PlaybackError>;
type PlaybackError = Box<dyn std::error::Error>;

#[tokio::main]
async fn main() -> PlaybackResult<()> {
    let app = build_app();

    let matches = app.get_matches();
    let filepath = matches.value_of("file");
    let access_log = matches.value_of("access_log");

    if filepath.is_none() & access_log.is_none() {
        println!("\x1b[01;31mError:\x1b[m please specify log filepath or access log text");
        std::process::exit(1)
    }

    let logs = if let Some(path) = filepath {
        resolve_log_file(path)
    } else if let Some(text) = access_log {
        resolve_log_text(text)
    } else {
        println!("\x1b[01;31mError:\x1b[m please specify log filepath or access log text");
        std::process::exit(1)
    }
    .unwrap();

    let shift = matches.value_of("shift").unwrap_or("0s");
    let shift_time = parse_time(shift)?;

    // TODO 新しいstructを作る
    // struct Hoge {
    //  request_time: time::Instant,
    //  request: Request
    // }
    // 的なやつ
    let shifted_logs = logs
        .iter()
        .map(|log| Log {
            accessed_at: log.accessed_at + Duration::from_std(shift_time).unwrap(),
            url: log.url.clone(),
            http_method: log.http_method.clone(),
            http_header: log.http_header.clone(),
            http_body: log.http_body.clone(),
        })
        .collect();

    send_requests(shifted_logs).await?;

    Ok(())
}

async fn send_requests(logs: Logs) -> PlaybackResult<()> {
    println!("start {:?}", logs);

    // TODO Add async task budget
    // const MAX_REQUEST: usize = 10_000;

    let mut tasks = vec![];
    for log in logs {
        let task = task::spawn(async move {
            schedule_request(log).await.unwrap();
        });
        tasks.push(task);
    }

    for task in tasks {
        task.await.unwrap();
    }

    Ok(())
}

async fn schedule_request(log: Log) -> PlaybackResult<()> {
    let duration = (log.accessed_at - chrono::Utc::now()).to_std()?;

    // TODO debug log
    println!("schedule for {:?}", duration);

    delay_for(duration).await;

    let mut request = reqwest::Request::new(log.http_method, log.url);
    *request.body_mut() = Some(log.http_body.into());
    *request.headers_mut() = (&log.http_header).try_into().unwrap();

    let response = reqwest::Client::new().execute(request).await;
    println!("{:?}", response);

    Ok(())
}

fn build_app() -> App<'static, 'static> {
    App::new(env!("CARGO_PKG_NAME"))
        .about(env!("CARGO_PKG_DESCRIPTION"))
        .version(env!("CARGO_PKG_VERSION"))
        .args(&[Arg::with_name("access_log")
            .help("log string")
            .required(false)])
        .arg(
            Arg::with_name("file")
                .help("Specifies access log filepath")
                .long("file")
                .short("f")
                .value_name("filepath"),
        )
        .arg(
            Arg::with_name("shift")
                .help("time shift (example 2s, 5m, 5h, 1d, 2w")
                .long("shift")
                .value_name("shift"),
        )
        .setting(AppSettings::DeriveDisplayOrder)
        .setting(AppSettings::ColoredHelp)
}

type Logs = Vec<Log>;

#[derive(Debug)]
struct Log {
    accessed_at: DateTime<Utc>,
    url: Url,
    http_method: Method,
    http_header: HashMap<String, String>,
    http_body: String,
}

type JsonLogs = Vec<JsonLog>;

#[derive(Serialize, Deserialize, Debug)]
struct JsonLog {
    accessed_at: String,
    url: String,
    http_method: String,
    http_header: HashMap<String, String>,
    http_body: String,
}

use std::convert::TryFrom;

impl TryFrom<JsonLog> for Log {
    type Error = anyhow::Error;

    fn try_from(json_log: JsonLog) -> Result<Self, Self::Error> {
        const FORMAT: &str = "%Y-%m-%d %H:%M:%S%.f UTC"; // TODO timezoneがUTCじゃなくても使えるようにする
        let dt = match NaiveDateTime::parse_from_str(&json_log.accessed_at, FORMAT) {
            Err(e) => bail!("Date and time format is not correct: {}", e),
            Ok(dt) => dt,
        };

        let accessed_at = DateTime::<Utc>::from_utc(dt, Utc);

        let url = match reqwest::Url::parse(&json_log.url) {
            Err(e) => bail!("url format is not correct: {}", e),
            Ok(url) => url,
        };

        let http_method = match Method::from_bytes(&json_log.http_method.as_bytes()) {
            Err(e) => bail!("Method is not correct: {}", e),
            Ok(url) => url,
        };

        let http_header = json_log.http_header;
        let http_body = json_log.http_body;

        Ok(Log {
            accessed_at,
            url,
            http_method,
            http_header,
            http_body,
        })
    }
}

fn resolve_log_file(log_file_path: &str) -> PlaybackResult<Logs> {
    let log_text = std::fs::read_to_string(log_file_path)?;

    resolve_log_text(&log_text)
}

fn resolve_log_text(log_text: &str) -> PlaybackResult<Logs> {
    let json_logs: JsonLogs = serde_json::from_str(log_text)?;

    let mut logs = vec![];
    for json_log in json_logs {
        let log = Log::try_from(json_log)?;

        logs.push(log)
    }

    Ok(logs)
}

#[test]
fn test_resolve_log_text() {
    let sample_json = include_str!("../log_examples/sample.json");

    assert!(resolve_log_text(sample_json).is_ok());
}

#[derive(Eq, PartialEq, Debug)]
enum TimeType {
    S(u64),
    M(u64),
    H(u64),
    D(u64),
    W(u64),
}

impl std::str::FromStr for TimeType {
    type Err = anyhow::Error;

    // TODO add error handle
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let tail = match s.chars().last() {
            None => bail!("invalid shift time"),
            Some(tail) => tail,
        };

        let number = s[0..(s.len() - 1)].parse()?;

        let time_type = match tail {
            's' => TimeType::S(number),
            'm' => TimeType::M(number),
            'h' => TimeType::H(number),
            'd' => TimeType::D(number),
            'w' => TimeType::W(number),
            _ => bail!("invalid shift time"),
        };

        Ok(time_type)
    }
}

///  parse time (example 2s, 5m, 5h, 1d, 2w")
#[test]
fn test_time_type_from_str() {
    // when valid
    let cases = vec![
        ("2s", TimeType::S(2)),
        ("5s", TimeType::S(5)),
        ("2m", TimeType::M(2)),
        ("2h", TimeType::H(2)),
        ("2d", TimeType::D(2)),
        ("2w", TimeType::W(2)),
    ];

    for (case, expect) in cases {
        let actual = TimeType::from_str(case).unwrap();

        assert_eq!(actual, expect)
    }

    // when invaid
    assert!(TimeType::from_str("").is_err());
    assert!(TimeType::from_str("2").is_err());
    assert!(TimeType::from_str("2t").is_err());
}

fn parse_time(s: &str) -> PlaybackResult<time::Duration> {
    let time_type = TimeType::from_str(s)?;

    Ok(match time_type {
        TimeType::S(t) => time::Duration::from_secs(t),
        TimeType::M(t) => time::Duration::from_secs(t * 60),
        TimeType::H(t) => time::Duration::from_secs(t * 60 * 60),
        TimeType::D(t) => time::Duration::from_secs(t * 60 * 60 * 24),
        TimeType::W(t) => time::Duration::from_secs(t * 60 * 60 * 24 * 7),
    })
}
