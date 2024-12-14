use {
    console::style,
    indicatif::{MultiProgress, ProgressBar, ProgressStyle},
    once_cell::sync::Lazy,
    std::sync::Arc,
    tap::prelude::*,
};

pub(crate) static PROGRESS_BAR: Lazy<MultiProgress> = Lazy::new(MultiProgress::new);

pub(crate) static VALIDATE_TOTAL_PROGRESS_BAR: Lazy<ProgressBar> = Lazy::new(|| {
    vertical_progress_bar(0, ProgressKind::Validate, indicatif::ProgressFinish::AndLeave)
        .attach_to(&PROGRESS_BAR)
        .tap_mut(|pb| pb.set_message("TOTAL"))
});

pub(crate) static DOWNLOAD_TOTAL_PROGRESS_BAR: Lazy<ProgressBar> = Lazy::new(|| {
    vertical_progress_bar(0, ProgressKind::Download, indicatif::ProgressFinish::AndLeave)
        .attach_to(&PROGRESS_BAR)
        .tap_mut(|pb| pb.set_message("TOTAL"))
});

pub(crate) static COPY_LOCAL_TOTAL_PROGRESS_BAR: Lazy<ProgressBar> = Lazy::new(|| {
    vertical_progress_bar(0, ProgressKind::Copy, indicatif::ProgressFinish::AndLeave)
        .attach_to(&PROGRESS_BAR)
        .tap_mut(|pb| pb.set_message("TOTAL"))
});

#[derive(Debug, Clone, Copy, derive_more::Display)]
pub enum ProgressKind {
    Validate,
    Download,
    Copy,
    Extract,
    InstallDirectives,
    ExtractTemporaryFile,
}

type ProgressBarPostAttach = Arc<dyn Fn(ProgressBar) -> ProgressBar + 'static>;

pub struct LazyProgressBar {
    bar: ProgressBar,
    post_attach: ProgressBarPostAttach,
}

impl LazyProgressBar {
    pub fn new(len: u64, post_attach: impl Fn(ProgressBar) -> ProgressBar + 'static) -> Self {
        Self {
            bar: ProgressBar::new(len),
            post_attach: Arc::new(post_attach),
        }
    }

    pub fn attach_to(self, multi_progress: &MultiProgress) -> ProgressBar {
        let Self { bar, post_attach } = self;
        let bar = multi_progress.add(bar);
        post_attach(bar)
    }
}

impl ProgressKind {
    fn color(self) -> &'static str {
        match self {
            ProgressKind::Validate => "yellow",
            ProgressKind::Download => "blue",
            ProgressKind::Copy => "cyan",
            ProgressKind::Extract => "magenta",
            ProgressKind::InstallDirectives => "green",
            ProgressKind::ExtractTemporaryFile => "white",
        }
    }
    #[rustfmt::skip]
    pub fn prefix(self) -> &'static str {
        match self {
            ProgressKind::Validate =>             "[ validate   ]",
            ProgressKind::Download =>             "[ download   ]",
            ProgressKind::Copy =>                 "[ copy       ]",
            ProgressKind::Extract =>              "[ extract    ]",
            ProgressKind::InstallDirectives =>    "[ directive  ]",
            ProgressKind::ExtractTemporaryFile => "[ temp extr. ]",
        }
    }
}

impl ProgressKind {
    pub fn stylize(self, pb: &mut ProgressBar) {
        let color = self.color();
        pb.set_style(
            ProgressStyle::with_template(&format!(
                "{{prefix:.bold}}▕{{bar:.{color}}}▏({{bytes}}/{{total_bytes}} {{bytes_per_sec}} ETA {{eta}}) {{msg:.{color}}}"
            ))
            .unwrap()
            .progress_chars("█▇▆▅▄▃▂▁  "),
        );
    }
}

pub fn vertical_progress_bar(len: u64, kind: ProgressKind, with_finish: indicatif::ProgressFinish) -> LazyProgressBar {
    LazyProgressBar::new(len, move |mut pb| {
        // pb.enable_steady_tick(std::time::Duration::from_millis(800));
        kind.stylize(&mut pb);
        let prefix = kind.prefix();
        pb.set_prefix(prefix);
        pb.enable_steady_tick(std::time::Duration::from_secs(1));
        pb.with_finish(with_finish.clone())
    })
}

// pub fn print_success(for_target: String, message: &str) {
//     PROGRESS_BAR
//         .println(format!("{} {}", style(for_target).bold().dim().green(), message))
//         .ok();
// }

// pub fn print_error(for_target: String, message: &anyhow::Error) {
//     let message = message
//         .chain()
//         .enumerate()
//         .try_fold(String::new(), |mut acc, (idx, next)| {
//             use std::fmt::Write;
//             acc.pipe_ref_mut(|acc| writeln!(acc, "{idx}. {next}", idx = idx + 1))
//                 .map(|_| acc)
//         })
//         .unwrap_or_else(|_| format!("{message:?}"));
//     PROGRESS_BAR
//         .println(format!("{} {}", style(for_target).bold().dim().red(), message))
//         .ok();
// }
// pub fn print_warn(for_target: &str, message: &anyhow::Error) {
//     PROGRESS_BAR
//         .println(format!("{} {message}", style(for_target).bold().dim().yellow(),))
//         .ok();
// }
