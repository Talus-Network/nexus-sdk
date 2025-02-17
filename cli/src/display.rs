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
        println!(
            "\n{arrow} {title}{separator}",
            arrow = "▶".bold().purple(),
            title = format!($($args)*).bold(),
            separator = $crate::display::separator()
        );
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

                loop {
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
        *self.success.lock().unwrap() = true;

        self.thread.join().unwrap();
    }

    /// Mark the loading as errored.
    pub(crate) fn error(self) {
        *self.error.lock().unwrap() = true;

        self.thread.join().unwrap();
    }
}
