use {
    console::style,
    indicatif::{MultiProgress, ProgressBar, ProgressStyle},
    once_cell::sync::Lazy,
    tap::prelude::*,
};

pub(crate) static PROGRESS_BAR: Lazy<MultiProgress> = Lazy::new(MultiProgress::new);
pub(crate) static VALIDATE_TOTAL_PROGRESS_BAR: Lazy<ProgressBar> = Lazy::new(|| {
    PROGRESS_BAR.add(vertical_progress_bar(0, ProgressKind::Validate).tap_mut(|pb| {
        pb.set_message("TOTAL");
    }))
});
pub(crate) static DOWNLOAD_TOTAL_PROGRESS_BAR: Lazy<ProgressBar> = Lazy::new(|| {
    PROGRESS_BAR.add(vertical_progress_bar(0, ProgressKind::Download).tap_mut(|pb| {
        pb.set_message("TOTAL");
    }))
});
pub(crate) static COPY_LOCAL_TOTAL_PROGRESS_BAR: Lazy<ProgressBar> = Lazy::new(|| {
    PROGRESS_BAR.add(vertical_progress_bar(0, ProgressKind::Copy).tap_mut(|pb| {
        pb.set_message("TOTAL");
    }))
});

#[derive(Debug, Clone, Copy, derive_more::Display)]
pub enum ProgressKind {
    Validate,
    Download,
    Copy,
}

impl ProgressKind {
    fn color(self) -> &'static str {
        match self {
            ProgressKind::Validate => "yellow",
            ProgressKind::Download => "blue",
            ProgressKind::Copy => "cyan",
        }
    }
    #[rustfmt::skip]
    pub fn prefix(self) -> &'static str {
        match self {
            ProgressKind::Validate => "[ validate ]",
            ProgressKind::Download => "[ download ]",
            ProgressKind::Copy =>     "[   copy   ]",
        }
    }
}
pub fn vertical_progress_bar(len: u64, kind: ProgressKind) -> ProgressBar {
    let color = kind.color();
    let prefix = kind.prefix();
    ProgressBar::new(len).tap_mut(|pb| {
        pb.enable_steady_tick(std::time::Duration::from_millis(800));
        pb.set_prefix(prefix);
        pb.set_style(
            ProgressStyle::with_template(&format!(
                "{{prefix:.bold}}▕{{bar:.{color}}}▏({{bytes}}/{{total_bytes}} {{bytes_per_sec}} ETA {{eta}}) {{msg:.{color}}}"
            ))
            .unwrap()
            .progress_chars("█▇▆▅▄▃▂▁  "),
        );
    })
}

pub fn print_error(for_target: &str, message: &anyhow::Error) {
    let message = message
        .chain()
        .enumerate()
        .try_fold(String::new(), |mut acc, (idx, next)| {
            use std::fmt::Write;
            acc.pipe_ref_mut(|acc| writeln!(acc, "{idx}. {next}", idx = idx + 1))
                .map(|_| acc)
        })
        .unwrap_or_else(|_| format!("{message:?}"));
    PROGRESS_BAR
        .println(format!("{} {}", style(for_target).bold().dim().red(), message))
        .ok();
}
// pub fn print_warn(for_target: &str, message: &anyhow::Error) {
//     PROGRESS_BAR
//         .println(format!("{} {message}", style(for_target).bold().dim().yellow(),))
//         .ok();
// }
