use {
    crate::prelude::*,
    colored::ColoredString,
    std::{
        sync::{Arc, Mutex},
        thread,
    },
};

/// Print a grey colored line to separate sections
pub(crate) fn separator() -> ColoredString {
    "\n-=-=-=-=-=-=-=-".truecolor(100, 100, 100)
}

/// Print the title of the currently executed command.
#[macro_export]
macro_rules! command_title {
    ($($args:tt)*) => {
        if !JSON_MODE.load(Ordering::Relaxed) {
            println!(
                "\n{arrow} {title}{separator}",
                arrow = "▶".bold().purple(),
                title = format!($($args)*).bold(),
                separator = $crate::display::separator()
            );
        }
    };
}

/// Ask the user for confirmation before proceeding.
#[macro_export]
macro_rules! confirm {
    ($($args:tt)*) => {
        {
            if !JSON_MODE.load(Ordering::Relaxed) {
                use std::io::{self, Write};

                print!("{warning} {message} {yn}: ", warning = "⚠".bold().yellow(), message = format!($($args)*).bold(), yn = "[y/N]".truecolor(100, 100, 100));

                io::stdout().flush().unwrap();

                let mut input = String::new();
                io::stdin().read_line(&mut input).unwrap();

                if input.trim().to_lowercase() != "y" {
                    std::process::exit(1);
                }
            }
        }
    };
}

/// Notify the user of a successful operation. Basicaly [`println!`] but
/// includes a not [`JSON_MODE`] check and some success formatting.
#[macro_export]
macro_rules! notify_success {
    ($($args:tt)*) => {
        if !JSON_MODE.load(Ordering::Relaxed) {
            println!(
                "[{check}] {msg}",
                check = "✔".green().bold(),
                msg = format!($($args)*)
            );
        }
    };
}

/// Similar to [`notify_success!`] but for errors.
#[macro_export]
macro_rules! notify_error {
    ($($args:tt)*) => {
        if !JSON_MODE.load(Ordering::Relaxed) {
            eprintln!(
                "[{ballot}] {msg}",
                ballot = "✘".red().bold(),
                msg = format!($($args)*)
            );
        }
    };
}

/// Formatted list item.
#[macro_export]
macro_rules! item {
    ($($args:tt)*) => {
        if !JSON_MODE.load(Ordering::Relaxed) {
            println!(
                "    {arrow} {item}",
                arrow = "▶".truecolor(100, 100, 100),
                item = format!($($args)*)
            );
        }
    };
}

/// Macro to print a loading state. Accepts a message and returns `success` and
/// `error` handles to change the state of the loading.
#[macro_export]
macro_rules! loading {
    ($fmt:expr) => {{
        use std::{
            io::Write,
            sync::{Arc, Mutex},
            thread,
        };

        let success = Arc::new(Mutex::new(false));
        let error = Arc::new(Mutex::new(false));

        let thread = {
            let success = success.clone();
            let error = error.clone();

            thread::spawn(move || {
                let frames = ["/", "-", "\\", "|"];

                let mut i = 0;

                while !JSON_MODE.load(Ordering::Relaxed) {
                    print!("\r[{}] {msg} ", frames[i].purple(), msg = format!($fmt));

                    if *success.lock().unwrap() {
                        println!(
                            "\r[{check}] {msg}",
                            check = "✔".green().bold(),
                            msg = format!($fmt)
                        );

                        break;
                    }

                    if *error.lock().unwrap() {
                        println!(
                            "\r[{ballot}] {msg}",
                            ballot = "✘".red().bold(),
                            msg = format!($fmt)
                        );

                        break;
                    }

                    i = (i + 1) % frames.len();

                    std::io::stdout().flush().unwrap();

                    thread::sleep(std::time::Duration::from_millis(100));
                }
            })
        };

        $crate::display::LoadingHandle::new(success, error, thread)
    }};
}

/// Struct helping with handling loading state.
pub(crate) struct LoadingHandle {
    success: Arc<Mutex<bool>>,
    error: Arc<Mutex<bool>>,
    thread: thread::JoinHandle<()>,
}

impl LoadingHandle {
    pub(super) fn new(
        success: Arc<Mutex<bool>>,
        error: Arc<Mutex<bool>>,
        thread: thread::JoinHandle<()>,
    ) -> Self {
        Self {
            success,
            error,
            thread,
        }
    }

    /// Mark the loading as successful.
    pub(crate) fn success(self) {
        if !JSON_MODE.load(Ordering::Relaxed) {
            *self.success.lock().unwrap() = true;

            self.thread.join().unwrap();
        }
    }

    /// Mark the loading as errored.
    pub(crate) fn error(self) {
        if !JSON_MODE.load(Ordering::Relaxed) {
            *self.error.lock().unwrap() = true;

            self.thread.join().unwrap();
        }
    }
}

/// If [`JSON_MODE`] is enabled, output the given data as JSON.
pub(crate) fn json_output<T: Serialize>(data: &T) -> AnyResult<(), NexusCliError> {
    if !JSON_MODE.load(Ordering::Relaxed) {
        return Ok(());
    }

    match serde_json::to_string_pretty(data) {
        Ok(json) => {
            println!("{}", json);

            Ok(())
        }
        Err(e) => Err(NexusCliError::Any(e.into())),
    }
}
