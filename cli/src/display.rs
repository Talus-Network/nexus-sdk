use {crate::prelude::*, colored::ColoredString};

/// Print a grey colored line to separate sections
pub(crate) fn separator() -> ColoredString {
    "\n-=-=-=-=-=-=-=-\n".truecolor(100, 100, 100)
}

/// Print the title of the currently executed command.
#[macro_export]
macro_rules! command_title {
    ($title:expr) => {
        println!(
            "{arrow} {title}{separator}",
            arrow = "▶".bold().purple(),
            title = format!($title).bold(),
            separator = separator()
        );
    };
}
