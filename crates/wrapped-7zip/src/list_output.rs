use {
    super::*,
    chrono::{NaiveDate, NaiveTime},
    std::{iter::empty, ops::Not, str::FromStr},
};

#[derive(Debug, PartialEq, Eq)]
pub struct ListOutputEntry {
    pub date: chrono::NaiveDate,
    pub time: chrono::NaiveTime,
    pub attr: String,
    pub size: u64,
    pub compressed: u64,
    pub name: PathBuf,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ListOutput {
    pub entries: Vec<ListOutputEntry>,
}

impl ListOutputEntry {
    pub fn from_str(archive_line: &str) -> Result<Self> {
        let mut entries = empty().chain(std::iter::successors(Some(archive_line), |archive_line| {
            archive_line
                .trim()
                .split_once(" ")
                .map(|(_, leftover)| leftover.trim())
        }));
        fn next_entry<T>(entry: &str) -> Result<T>
        where
            T: FromStr,
            <T as FromStr>::Err: std::fmt::Debug,
        {
            entry
                .split_whitespace()
                .next()
                .context("empty line")
                .and_then(|v| {
                    v.parse()
                        .map_err(|e| anyhow!("bad value: {e:#?}"))
                        .context("could not parse value")
                })
        }
        Ok(ListOutputEntry {
            date: entries.next().context("no date").and_then(|date| {
                date.pipe(next_entry)
                    .with_context(|| format!("bad date: '{date}'"))
            })?,
            time: entries.next().context("no time").and_then(|time| {
                time.pipe(next_entry)
                    .with_context(|| format!("bad time: '{time}'"))
            })?,
            attr: entries.next().context("no attr").and_then(|attr| {
                attr.pipe(next_entry)
                    .with_context(|| format!("bad attr: '{attr}'"))
            })?,
            size: entries.next().context("no size").and_then(|size| {
                size.pipe(next_entry)
                    .with_context(|| format!("bad size: '{size}'"))
            })?,
            compressed: entries
                .next()
                .context("no compressed")
                .and_then(|compressed| {
                    compressed
                        .pipe(next_entry)
                        .with_context(|| format!("bad compressed: '{compressed}'"))
                })?,
            name: entries
                .next()
                .context("no file name")
                .map(|e| PathBuf::from(e))?,
        })
    }
}

impl ListOutput {
    pub fn from_str(output: &str) -> Result<Self> {
        let output = output.trim();
        let total_lines = output.lines().count();
        output
            .lines()
            .take(total_lines.saturating_sub(2))
            .map(ListOutputEntry::from_str)
            .skip_while(Result::is_err)
            .collect::<Result<Vec<_>>>()
            .map(|entries| ListOutput { entries })
            .with_context(|| format!("parsing output\n{output}"))
    }
}

#[cfg(test)]
mod tests;
