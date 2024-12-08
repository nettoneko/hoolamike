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
    pub compressed: Option<u64>,
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

        let mut parse_entry = || -> Result<_> {
            let date = entries.next().context("no date").and_then(|date| {
                date.pipe(next_entry)
                    .with_context(|| format!("bad date: '{date}'"))
            })?;
            let time = entries.next().context("no time").and_then(|time| {
                time.pipe(next_entry)
                    .with_context(|| format!("bad time: '{time}'"))
            })?;
            let attr = entries.next().context("no attr").and_then(|attr| {
                attr.pipe(next_entry)
                    .with_context(|| format!("bad attr: '{attr}'"))
            })?;
            let size = entries.next().context("no size").and_then(|size| {
                size.pipe(next_entry)
                    .with_context(|| format!("bad size: '{size}'"))
            })?;

            entries
                .next()
                .context("no compressed/name")
                .and_then(|maybe_compressed| -> Result<_> {
                    match maybe_compressed
                        .pipe(next_entry::<u64>)
                        .map_err(|_| maybe_compressed)
                    {
                        Ok(compressed) => Ok(ListOutputEntry {
                            date,
                            time,
                            attr,
                            size,
                            compressed: Some(compressed),
                            name: entries
                                .next()
                                .context("no file name")
                                .map(ToOwned::to_owned)
                                .map(MaybeWindowsPath)
                                .map(MaybeWindowsPath::into_path)?,
                        }),
                        Err(file_name) => Ok(ListOutputEntry {
                            date,
                            time,
                            attr,
                            size,
                            compressed: None,
                            name: file_name
                                .pipe(ToOwned::to_owned)
                                .pipe(MaybeWindowsPath)
                                .pipe(MaybeWindowsPath::into_path),
                        }),
                    }
                })
        };

        parse_entry()
            .tap_err(|e| tracing::trace!("discarding\n{archive_line}\nreason:{e:#?}"))
            .with_context(|| format!("when parsing line from\n'{archive_line}'"))
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
            .tap_ok(|entries| tracing::trace!("found entries: {entries:#?}\n\nn output: {output}"))
    }
}

#[cfg(test)]
mod tests;
