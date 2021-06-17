#![allow(missing_docs)]
use std::path::Path;

#[derive(Debug, Clone, Copy, Ord, PartialOrd, Eq, PartialEq)]
pub struct Retries {
    /// How often to retry if an interrupt happens.
    pub on_interrupt: usize,
    /// How many directories can be created in total. 1 means only the target directory itself can be created and
    /// not a single parent directory.
    /// Note that this also counts towards retries needed to combat racy behaviour from other
    /// processes trying to delete empty directories.
    pub on_create_directory: usize,
}

impl Default for Retries {
    fn default() -> Self {
        Retries {
            on_interrupt: 10,
            on_create_directory: 100,
        }
    }
}

mod error {
    use quick_error::quick_error;
    use std::path::PathBuf;

    quick_error! {
        #[derive(Debug)]
        pub enum Error {
            Intermediate(kind: std::io::ErrorKind) {
                display("Intermediate failure with error: {:?}", kind)
                from()
            }
            Permanent { err: std::io::Error, dir: PathBuf, attempts: Option<usize> } {
                display("Permanently failing to create directory {:?}{}", dir, match attempts {Some(attempts) => format!(" after {} attempts", attempts), None => "".into()})
                source(err)
            }
        }
    }

    impl Error {
        pub fn intermediate(&self) -> Option<std::io::ErrorKind> {
            match self {
                Error::Intermediate(kind) => Some(*kind),
                _ => None,
            }
        }
    }
}
pub use error::Error;

/// A special iterator which communicates its operation through results where…
///
/// * `Some(Ok(created_directory))` is yielded once or more success, followed by `None`
/// * `Some(Err(Error::Intermediate))` is yielded zero or more times while trying to create the directory.
/// * `Some(Err(Error::Permanent))` is yielded exactly once on failure.
pub struct Iter<'a> {
    cursors: Vec<&'a Path>,
    retries: Retries,
    original_retries: Retries,
}

impl<'a> Iter<'a> {
    pub fn new(target: &'a Path) -> Self {
        let retries = Default::default();
        Iter {
            cursors: vec![target],
            original_retries: retries,
            retries,
        }
    }

    pub fn new_with_retries(target: &'a Path, retries: Retries) -> Self {
        Iter {
            cursors: vec![target],
            original_retries: retries,
            retries,
        }
    }

    fn pernanent_failure(
        &mut self,
        dir: &Path,
        err: impl Into<std::io::Error>,
        attempts: impl Into<Option<usize>>,
    ) -> Option<Result<&'a Path, Error>> {
        self.cursors.clear();
        Some(Err(Error::Permanent {
            err: err.into(),
            dir: dir.to_owned(),
            attempts: attempts.into(),
        }))
    }

    fn intermediate_failure(&self, err: std::io::Error) -> Option<Result<&'a Path, Error>> {
        Some(Err(Error::Intermediate(err.kind())))
    }
}

impl<'a> Iterator for Iter<'a> {
    type Item = Result<&'a Path, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        use std::io::ErrorKind::*;
        match self.cursors.pop() {
            Some(dir) => match std::fs::create_dir(dir) {
                Ok(()) => Some(Ok(dir)),
                Err(err) => match err.kind() {
                    AlreadyExists if dir.is_dir() => Some(Ok(dir)),
                    AlreadyExists => self.pernanent_failure(dir, err, None),
                    NotFound if self.retries.on_create_directory <= 1 => {
                        self.pernanent_failure(dir, NotFound, self.original_retries.on_create_directory)
                    }
                    NotFound => {
                        self.retries.on_create_directory -= 1;
                        self.cursors.push(dir);
                        self.cursors.push(match dir.parent() {
                            None => return self.pernanent_failure(dir, InvalidInput, 1),
                            Some(parent) => parent,
                        });
                        self.intermediate_failure(err)
                    }
                    Interrupted if self.retries.on_interrupt <= 1 => {
                        self.pernanent_failure(dir, Interrupted, self.original_retries.on_interrupt)
                    }
                    Interrupted => {
                        self.retries.on_interrupt -= 1;
                        self.cursors.push(dir);
                        self.intermediate_failure(err)
                    }
                    kind => todo!("{:?}", kind),
                },
            },
            None => None,
        }
    }
}
