//! Structured logging utilities.

use std::fmt;
use std::sync::Once;
use time::macros::format_description;
use time::OffsetDateTime;

use tracing::Subscriber;
use tracing_subscriber::fmt::format::{FormatEvent, FormatFields, Writer};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::FmtContext;
use tracing_subscriber::registry::LookupSpan;

pub struct LoguruFormatter;

const TIMESTAMP_FORMAT: &[time::format_description::FormatItem<'static>] =
    format_description!("[year]-[month]-[day] [hour]:[minute]:[second].[subsecond digits:9]");

impl<S, N> FormatEvent<S, N> for LoguruFormatter
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &tracing::Event<'_>,
    ) -> fmt::Result {
        let now = OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc());
        let timestamp = now
            .format(TIMESTAMP_FORMAT)
            .unwrap_or_else(|_| "invalid-time".to_string());
        let metadata = event.metadata();
        let file = metadata.file().unwrap_or("unknown");
        let line = metadata.line().unwrap_or(0);
        let thread_id = format!("{:?}", std::thread::current().id());
        let level_display = format_level(metadata.level());

        write!(
            writer,
            "{} | {:<5} | {}:{} | thread={} | ",
            timestamp,
            level_display,
            file,
            line,
            thread_id,
        )?;

        ctx.field_format().format_fields(writer.by_ref(), event)?;
        writeln!(writer)
    }
}

fn format_level(level: &tracing::Level) -> String {
    let use_color = std::env::var("NO_COLOR").is_err();
    let level_str = level.as_str();
    let padded = format!("{:<5}", level_str);
    if !use_color {
        return padded;
    }
    let color = match *level {
        tracing::Level::ERROR => "31",
        tracing::Level::WARN => "33",
        tracing::Level::INFO => "32",
        tracing::Level::DEBUG => "34",
        tracing::Level::TRACE => "90",
    };
    format!("\x1b[{}m{}\x1b[0m", color, padded)
}

pub fn setup_log() -> Result<(), tracing::dispatcher::SetGlobalDefaultError> {
    static INIT: Once = Once::new();
    let mut result = Ok(());

    INIT.call_once(|| {
        let filter =
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
        let subscriber = tracing_subscriber::fmt()
            .event_format(LoguruFormatter)
            .with_env_filter(filter)
            .finish();

        result = tracing::subscriber::set_global_default(subscriber);
    });

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;
    use std::sync::{Arc, Mutex};
    use tracing_subscriber::fmt::MakeWriter;

    #[derive(Clone, Default)]
    struct TestWriter {
        buffer: Arc<Mutex<Vec<u8>>>,
    }

    impl<'a> MakeWriter<'a> for TestWriter {
        type Writer = TestWriterGuard;

        fn make_writer(&'a self) -> Self::Writer {
            TestWriterGuard {
                buffer: self.buffer.clone(),
            }
        }
    }

    struct TestWriterGuard {
        buffer: Arc<Mutex<Vec<u8>>>,
    }

    impl io::Write for TestWriterGuard {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            let mut buffer = self.buffer.lock().expect("writer lock");
            buffer.extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn formatter_writes_expected_fields() {
        let writer = TestWriter::default();
        let buffer = writer.buffer.clone();

        let subscriber = tracing_subscriber::fmt()
            .event_format(LoguruFormatter)
            .with_writer(writer)
            .finish();

        tracing::subscriber::with_default(subscriber, || {
            tracing::info!("test message");
        });

        let output = String::from_utf8(buffer.lock().expect("buffer lock").clone())
            .expect("utf8 log");
        assert!(output.contains("INFO"));
        assert!(output.contains("test message"));
        assert!(output.contains("logging.rs"));
    }
}
