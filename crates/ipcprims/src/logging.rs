use clap::ValueEnum;

#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum LogFormat {
    Text,
    Json,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl LogLevel {
    pub fn as_filter(self) -> tracing::level_filters::LevelFilter {
        match self {
            LogLevel::Error => tracing::level_filters::LevelFilter::ERROR,
            LogLevel::Warn => tracing::level_filters::LevelFilter::WARN,
            LogLevel::Info => tracing::level_filters::LevelFilter::INFO,
            LogLevel::Debug => tracing::level_filters::LevelFilter::DEBUG,
            LogLevel::Trace => tracing::level_filters::LevelFilter::TRACE,
        }
    }
}

pub fn init_logging(format: LogFormat, level: LogLevel) {
    let builder = tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_max_level(level.as_filter())
        .with_ansi(false)
        .with_target(false);

    match format {
        LogFormat::Text => {
            let _ = builder.try_init();
        }
        LogFormat::Json => {
            let _ = builder.json().try_init();
        }
    }
}
