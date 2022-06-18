use std::{
  cmp::Ordering,
  fmt::Display,
  fs::read_dir,
  io::Error,
  ops::{Add, AddAssign},
  path::{Path, PathBuf},
};

#[derive(Clone, Copy, Eq, PartialEq, PartialOrd, Ord)]
pub struct FileSize(u64);

impl Display for FileSize {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    const LETTER: &str = "ETGMK";
    let mut var = (' ', 0);
    for order in 0..5 {
      if self.0 > (1 << ((5 - order) * 10)) {
        var = (LETTER.chars().nth(order).unwrap(), (5 - order) * 10);
        break;
      }
    }
    f.write_fmt(format_args!("{}{}", self.0 >> var.1, var.0))
  }
}

impl From<u64> for FileSize {
  fn from(size: u64) -> Self {
    FileSize(size)
  }
}

impl From<FileSize> for u64 {
  fn from(size: FileSize) -> Self {
    size.0
  }
}

impl Add<u64> for FileSize {
  type Output = FileSize;

  fn add(self, rhs: u64) -> Self::Output {
    (self.0 + rhs).into()
  }
}

impl AddAssign<u64> for FileSize {
  fn add_assign(&mut self, rhs: u64) {
    self.0 += rhs
  }
}

impl AddAssign<Self> for FileSize {
  fn add_assign(&mut self, rhs: Self) {
    self.0 += rhs.0
  }
}

pub struct Node {
  pub size: FileSize,
  pub name: PathBuf,
  pub children: Vec<Node>,
  pub is_directory: bool,
  pub is_marked: bool,
}

impl Node {
  pub fn delete_marked(&self) {
    for child in self.children.iter() {
      if child.is_marked {
        if child.name.is_dir() {
          if let Err(why) = std::fs::remove_dir_all(&child.name) {
            eprintln!(
              "Failed to remove directory \"{}\": {}",
              child.name.display(),
              why
            );
          }
        } else {
          if let Err(why) = std::fs::remove_file(&child.name) {
            eprintln!(
              "Failed to remove file \"{}\": {}",
              child.name.display(),
              why
            );
          }
        }
      } else {
        child.delete_marked()
      }
    }
  }
  pub fn mark(&mut self, mark: bool) {
    self.is_marked = mark;
    for child in self.children.iter_mut() {
      child.mark(mark);
    }
  }
  pub fn find_node_mut<'a>(&'a mut self, path: &Path) -> Option<&'a mut Node> {
    let self_path = Path::new(&self.name);
    if self_path == path {
      return Some(self);
    }
    for child in self.children.iter_mut() {
      let child_path = Path::new(&child.name);
      if child_path == path {
        return Some(child);
      }
      if path.starts_with(child_path) {
        return child.find_node_mut(path);
      }
    }
    None
  }

  pub fn find_node<'a>(&'a self, path: &Path) -> Option<&'a Node> {
    let self_path = Path::new(&self.name);
    if self_path == path {
      return Some(self);
    }
    for child in self.children.iter() {
      let child_path = Path::new(&child.name);
      if child_path == path {
        return Some(child);
      }
      if path.starts_with(child_path) {
        return child.find_node(path);
      }
    }
    None
  }
  pub fn new(size: FileSize, name: PathBuf, is_directory: bool) -> Self {
    return Self {
      size,
      name,
      children: Vec::new(),
      is_directory,
      is_marked: false,
    };
  }
}

pub struct Callbacks<'a> {
  pub failed_meta: &'a dyn Fn(Error, &Path),
  pub failed_entry: &'a dyn Fn(Error, &Path),
  pub failed_inspections: &'a dyn Fn(Error, &Path),
  pub indexed_bytes: &'a mut dyn FnMut(u64),
}

pub fn inspect_dir(path: &Path, callbacks: &mut Callbacks) -> std::io::Result<Node> {
  const SPECIAL_PATHS: [&str; 3] = ["/dev", "/proc", "/mnt"];
  for spec_path in SPECIAL_PATHS {
    if path.starts_with(spec_path) {
      return Ok(Node::new(0.into(), path.to_owned(), true));
    }
  }
  let meta = path.metadata()?;
  let mut node = Node::new(meta.len().into(), path.to_owned(), meta.is_dir());
  for entry in read_dir(path)? {
    let entry = match entry {
      Ok(dir_entry) => dir_entry,
      Err(why) => {
        (callbacks.failed_entry)(why, path);
        continue;
      }
    };
    let meta = match entry.metadata() {
      Ok(meta) => meta,
      Err(why) => {
        (callbacks.failed_meta)(why, &entry.path());
        continue;
      }
    };
    if meta.is_file() {
      node.size += meta.len();
      node
        .children
        .push(Node::new(meta.len().into(), entry.path(), false));
    } else if meta.is_dir() {
      match inspect_dir(&entry.path(), callbacks) {
        Ok(child) => {
          (callbacks.indexed_bytes)(child.size.into());
          node.size += child.size;
          node.children.push(child);
        }
        Err(why) => {
          (callbacks.failed_inspections)(why, &entry.path());
          continue;
        }
      }
    }
  }
  node.children.sort_by(|a, b| {
    if a.is_directory && b.is_directory {
      return a.name.cmp(&b.name);
    }
    if a.is_directory {
      return Ordering::Less;
    }
    if b.is_directory {
      return Ordering::Greater;
    }
    a.name.cmp(&b.name)
  });
  Ok(node)
}
