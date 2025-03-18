use {
    super::*,
    chrono::NaiveDateTime,
    std::{collections::BTreeMap, ops::Not, str::FromStr},
};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ListOutputEntry {
    pub modified: chrono::NaiveDateTime,
    pub original_path: String,
    pub created: Option<chrono::NaiveDateTime>,
    pub size: u64,
    pub path: PathBuf,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ListOutput {
    pub entries: Vec<ListOutputEntry>,
}

fn parse_date(input: &str) -> Result<NaiveDateTime> {
    input
        .split_once(".")
        .map(|(date, _subsec)| date)
        .unwrap_or(input)
        .pipe(|input| NaiveDateTime::parse_from_str(input, "%Y-%m-%d %H:%M:%S").with_context(|| format!("not a valid date: [{input}]")))
}

#[cfg(test)]
mod test_date_parsing {
    use super::*;
    #[test]
    fn test_example_1() -> Result<()> {
        parse_date("2024-08-04 22:02:17.2575336").map(|_| ())
    }

    #[test]
    fn test_example_2() -> Result<()> {
        parse_date("2024-08-06 13:25:23.4918567").map(|_| ())
    }
}

impl FromStr for ListOutput {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        s.trim().to_string().pipe_ref(|trimmed| {
            trimmed
                .split_once("----------")
                .context("no indicator")
                .and_then(|(_header, files)| {
                    files
                        .split("\n\n")
                        .filter_map(|entry| {
                            entry
                                .trim()
                                .pipe(|trimmed| trimmed.is_empty().not().then_some(trimmed))
                        })
                        .filter(|entry| entry.lines().count() > 2)
                        .map(|entry| {
                            entry
                                .trim()
                                .lines()
                                .map(|line| {
                                    line.split_once("=")
                                        .context("no attribute indicator (=)")
                                        .map(|(k, v)| (k.trim(), v.trim()))
                                        .context(line.to_string())
                                })
                                .collect::<Result<BTreeMap<_, _>>>()
                                .map(|e| {
                                    e.into_iter()
                                        .filter(|(_, v)| v.is_empty().not())
                                        .filter(|(_, v)| v != &"-")
                                        .collect::<BTreeMap<_, _>>()
                                })
                                .and_then(|mut entry| -> Result<_> {
                                    let path = entry.remove("Path").context("no such field")?.to_string();
                                    Ok(ListOutputEntry {
                                        created: entry
                                            .remove("Created")
                                            .map(parse_date)
                                            .transpose()
                                            .context("Created")?,
                                        modified: entry
                                            .remove("Modified")
                                            .context("no such field")
                                            .and_then(parse_date)
                                            .context("Modified")?,
                                        size: entry
                                            .remove("Size")
                                            .context("no such field")
                                            .and_then(|v| v.parse().context("bad value"))
                                            .context("Size")?,
                                        original_path: path.clone(),
                                        path: path
                                            .pipe(MaybeWindowsPath)
                                            .pipe(MaybeWindowsPath::into_path),
                                    })
                                })
                                .context(entry.to_string())
                        })
                        .collect::<Result<Vec<_>>>()
                        .map(|entries| Self { entries })
                })
        })
    }
}
