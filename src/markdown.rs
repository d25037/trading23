use std::{fmt::Write, path::Path};

use crate::my_error::MyError;

pub struct Markdown {
    buffer: String,
}
impl Markdown {
    pub fn new() -> Self {
        Markdown {
            buffer: String::new(),
        }
    }
    pub fn h1(&mut self, text: &str) -> Result<(), MyError> {
        writeln!(&mut self.buffer, "# {}", text)?;
        Ok(())
    }
    pub fn h2(&mut self, text: &str) -> Result<(), MyError> {
        writeln!(&mut self.buffer, "## {}", text)?;
        Ok(())
    }
    pub fn body(&mut self, text: &str) -> Result<(), MyError> {
        writeln!(&mut self.buffer, "{}", text)?;
        Ok(())
    }

    // pub fn append(&mut self, markdown: Markdown) {
    //     self.buffer.push_str(&markdown.buffer);
    // }

    pub fn buffer(&self) -> &str {
        &self.buffer
    }

    pub fn write_to_file(&self, path: &Path) -> Result<(), MyError> {
        // create parent directory if not exists
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)?;
            }
        }

        let path_with_extension = path.with_extension("md");
        std::fs::write(path_with_extension, &self.buffer)?;
        Ok(())
    }
}
