use chrono::{DateTime, Datelike, Duration, NaiveDateTime, Timelike};
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::io::Error as IoError;
use std::io::ErrorKind as IoErrorKind;
use std::io::Write;
use std::ops::Add;
use std::path::PathBuf;
use std::time::SystemTime;

const DATE_FORMAT: &str = "%Y-%m-%d_%H:%M:%S%.3f";

struct Log {
    file: PathBuf,
    project: String,
    stage_no: u32,
    begin: NaiveDateTime,
    end: NaiveDateTime,
}

#[derive(Clone)]
struct UsageDay {
    at_minute: Vec<usize>,
    at_hour: Vec<usize>,
}

struct Usage {
    days: [UsageDay; 7],
}

impl Default for Usage {
    fn default() -> Self {
        let day = UsageDay {
            at_minute: vec![0; 24 * 60],
            at_hour: vec![0; 24],
        };
        Usage {
            days: [
                day.clone(),
                day.clone(),
                day.clone(),
                day.clone(),
                day.clone(),
                day.clone(),
                day,
            ],
        }
    }
}

fn main() {
    let mut active = Usage::default();
    let mut starting = Usage::default();

    let files = get_log_files();
    let mut logs = HashMap::<String, Vec<Log>>::default();

    for file in files {
        match read_log_file(&file) {
            Ok(log) => {
                add_active(&mut active, &log);
                add_starting(&mut starting, &log);
                match logs.entry(log.project.clone()) {
                    Entry::Occupied(mut entry) => entry.get_mut().push(log),
                    Entry::Vacant(mut entry) => {
                        entry.insert(vec![log]);
                    }
                };
            }
            Err(e) => {
                eprintln!("{}", e);
            }
        }
    }
    print(&active);
    print(&starting);

    write_csv(&active, "../active.csv");
    write_csv(&starting, "../starting.csv");

    let auto = find_auto(logs);

    let mut auto_active = Usage::default();
    let mut auto_starting = Usage::default();

    for log in &auto {
        add_active(&mut auto_active, &log);
        add_starting(&mut auto_starting, &log);
    }

    write_csv(&auto_active, "../auto_active.csv");
    write_csv(&auto_starting, "../auto_starting.csv");
}

fn find_auto(map: HashMap<String, Vec<Log>>) -> Vec<Log> {
    let mut vec = Vec::default();
    for (_project, mut logs) in map.into_iter() {
        logs.sort_by(|a, b| a.stage_no.cmp(&b.stage_no));
        let mut iter = logs.into_iter();
        let mut prev = iter.next().unwrap().end;
        for log in iter {
            let auto = log.begin - prev < Duration::minutes(1);
            prev = log.end;
            if auto {
                vec.push(log);
            }
        }
    }
    vec
}

fn print(usage: &Usage) {
    eprintln!(
        "      {}",
        (0..24)
            .into_iter()
            .map(|h| format!("{:4}", h))
            .collect::<Vec<String>>()
            .join(" ")
    );
    eprintln!(
        "    +{}",
        (0..24)
            .into_iter()
            .map(|_| "----")
            .collect::<Vec<_>>()
            .join(" ")
    );
    for n in 0..7_usize {
        let day = match n {
            0 => "Mo",
            1 => "Tu",
            2 => "We",
            3 => "Th",
            4 => "Fr",
            5 => "Sa",
            6 => "Su",
            i => panic!("A week has only seven days, but given index {}", i),
        };
        eprintln!(
            " {} | {}",
            day,
            usage.days[n]
                .at_hour
                .iter()
                .map(|h| format!("{:4}", h))
                .collect::<Vec<String>>()
                .join(" "),
        );
    }
}

fn add_active(usage: &mut Usage, log: &Log) {
    let duration = log.end.signed_duration_since(log.begin);
    for i in 0..=duration.num_minutes() {
        let time = log.begin.add(Duration::minutes(i));
        let day_index = time.weekday().num_days_from_monday();
        let minute = time.num_seconds_from_midnight() / 60;
        usage.days[day_index as usize].at_minute[minute as usize] += 1;
    }
    for i in 0..=duration.num_hours() {
        let time = log.begin.add(Duration::hours(i));
        let day_index = time.weekday().num_days_from_monday();
        let hour = time.num_seconds_from_midnight() / 60 / 60;
        usage.days[day_index as usize].at_hour[hour as usize] += 1;
    }
}

fn add_starting(usage: &mut Usage, log: &Log) {
    let day_index = log.begin.weekday().num_days_from_monday();
    let minute = log.begin.num_seconds_from_midnight() / 60;
    let hour = log.begin.num_seconds_from_midnight() / 60 / 60;
    usage.days[day_index as usize].at_minute[minute as usize] += 1;
    usage.days[day_index as usize].at_hour[hour as usize] += 1;
}

fn write_csv(usage: &Usage, path: &str) {
    let mut file = std::fs::File::create(path).unwrap();
    writeln!(file, "day;hour;value").unwrap();
    for d in 0..usage.days.len() {
        for (i, v) in usage.days[d].at_hour.iter().enumerate() {
            writeln!(file, "{};{};{}", d + 1, i, v).unwrap();
        }
    }
}

fn read_log_file(path: &PathBuf) -> Result<Log, IoError> {
    let mut lines = std::fs::read_to_string(path)?;
    let mut lines = lines.lines();
    let first = lines
        .next()
        .ok_or_else(|| IoError::from(IoErrorKind::UnexpectedEof))?
        .split_whitespace()
        .next()
        .ok_or_else(|| IoErrorKind::UnexpectedEof)?;
    let begin = NaiveDateTime::parse_from_str(first, DATE_FORMAT);
    let begin = begin.map_err(|_| IoError::from(IoErrorKind::InvalidInput))?;
    let last = lines
        .last()
        .ok_or_else(|| IoError::from(IoErrorKind::UnexpectedEof))?
        .split_whitespace()
        .next()
        .ok_or_else(|| IoErrorKind::UnexpectedEof)?;
    let end = NaiveDateTime::parse_from_str(last, DATE_FORMAT)
        .map_err(|_| IoError::from(IoErrorKind::InvalidInput))?;

    let file_name = path.file_name().unwrap().to_string_lossy().to_string();
    let mut split = file_name.split("_");
    let project = split.next().unwrap().to_string();
    let stage_no = split.next().unwrap().parse::<u32>().unwrap();

    Ok(Log {
        file: path.clone(),
        project,
        stage_no,
        begin,
        end,
    })
}

fn get_log_files() -> Vec<PathBuf> {
    let logs_dir_or_files = std::env::args().skip(1).collect::<Vec<_>>();
    let log_files = if logs_dir_or_files.len() > 1 {
        logs_dir_or_files.into_iter().map(PathBuf::from).collect()
    } else if logs_dir_or_files.len() == 1 {
        let path = &logs_dir_or_files[0];
        let dir = std::fs::read_dir(path).expect("Failed to read directory");
        let mut files = Vec::default();
        for entry in dir {
            let entry = entry.expect("Failed to read directory entry");
            let path = PathBuf::from(path).join(entry.file_name().to_string_lossy().to_string());
            files.push(path);
        }
        files
    } else {
        panic!("Require path to logs directory or files")
    };
    log_files
}
