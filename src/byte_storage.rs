use std::{fmt::Display, path::{PathBuf, MAIN_SEPARATOR}, str::FromStr};


use crate::arena::Arena;

const ALLOC_SIZE: usize = 100_000;
// const ALLOC_SIZE: usize = 1000;

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct FilePath {
    pub data: &'static [u8],
}

impl FilePath {
    pub fn new(data: &'static [u8]) -> Self {
        Self {
            data,
        }
    }

    pub fn name(&self) -> &str {
        let Some(last) = self.into_iter().last() else { return "" };
        unsafe { std::str::from_utf8_unchecked(last) }
    }

    pub fn path(&self) -> &str {
        unsafe { std::str::from_utf8_unchecked(self.data) }
    }

    pub fn file_ext(&self) -> &str {
        for (ch, i) in self.data.iter().rev().zip((0..self.data.len()).rev()) {
            if *ch as char == MAIN_SEPARATOR {
                break;
            }
            if *ch == b'.' {
                return unsafe { std::str::from_utf8_unchecked(&self.data[(i+1)..]) };
            }
        }
        ""
    }

    pub fn containing_folder(&self) -> String {
        let Ok(mut path) =  PathBuf::from_str(self.path()) else { return "".into() };
        if path.is_dir() {
            return self.clone().into()
        }
        if path.pop() {
            return path.to_string_lossy().to_string()
        }

        "".into()
    }
}

pub struct FilePathIterator {
    index: usize,
    inner: &'static [u8],
}

impl Iterator for FilePathIterator {
    type Item = &'static [u8];

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.inner.len() - 1 {
            return None;
        }
        self.index += 1;
        let old_index = self.index;
        while self.index < self.inner.len() && self.inner[self.index] != MAIN_SEPARATOR as u8 {
            self.index += 1;
        }
        Some(&self.inner[old_index..self.index])
    }
}

impl IntoIterator for FilePath {
    type Item = &'static [u8];

    type IntoIter = FilePathIterator;

    fn into_iter(self) -> Self::IntoIter {
        FilePathIterator { index: 0, inner: self.data }
    }
}

impl Display for FilePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} -- {}", self.name(), self.path())
    }
}

impl Into<String> for FilePath {
    fn into(self) -> String {
        self.path().into()
    }
}

//pub struct NewRefStorage {
//    data: parking_lot::RwLock<Arena<FilePath>>,
//}
//
//impl NewRefStorage {
//    pub fn new() -> Self {
//        Self {
//            data: parking_lot::RwLock::new(Arena::new()),
//        }
//    }
//
//    pub fn extend<I>(&self, data: I) 
//    where
//        I: ExactSizeIterator<Item = FilePath>,
//    {
//        self.data.write().extend(data);
//    }
//}

struct RefStorageIterator<'a> {
    index: usize,
    data: &'a parking_lot::RwLock<Arena<FilePath>>,
}

impl<'a> Iterator for RefStorageIterator<'a> {
    type Item = FilePath;

    fn next(&mut self) -> Option<Self::Item> {
        todo!()
    }
}

//impl<'a> IntoIterator for NewRefStorage {
//    type Item = FilePath;
//    type IntoIter = RefStorageIterator<'a>;
//
//    fn into_iter(self) -> Self::IntoIter {
//        RefStorageIterator {
//            index: 0,
//            data: &self.data,
//        }
//    }
//}


// pub struct ByteStorage {
//     pub arena: Arena<u8>,
//     ref_storage: Arc<RefStorage>,
// }
// 
// impl ByteStorage {
//     pub fn new(ref_storage: Arc<RefStorage>) -> Self {
//         Self {
//             ref_storage,
//             arena: Arena::new(),
//         }
//     }
// 
//     pub fn extend(&mut self, paths: Vec<PathBuf>) {
//         let mut last = self.allocs.last_mut().unwrap();
//         let mut previous_index = last.len();
//         let mut references = Vec::with_capacity(paths.len());
// 
//         for p in &paths {
//             let len = p.as_os_str().as_bytes().len();
//             let bytes = p.as_os_str().as_bytes();
//             if last.capacity() < last.len() + len {
//                 self.allocs.push(Vec::with_capacity(ALLOC_SIZE));
//                 last = self.allocs.last_mut().unwrap();
//                 previous_index = 0;
//             }
//             last.extend_from_slice(p.as_os_str().as_bytes());
//             let r = unsafe { &*slice_from_raw_parts(&last[previous_index], len) };
//             references.push(FilePath::new(r));
//             previous_index = last.len();
//         }
// 
//         self.ref_storage.extend(references);
//     }
// }


#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    #[test]
    fn test_iterator() {
        let path = "/home/paavo/.config/i3/config";
        let input = "i3";

        let file_path = FilePath::new(path.as_bytes());

        let parts = file_path.into_iter().collect::<Vec<_>>();
        assert_eq!(parts, vec![b"home".as_ref(), b"paavo", b".config", b"i3", b"config"]);
    }

    // #[test]
    // fn test_bytestorage() {
    //     let path = "/path/to/some/file";

    //     let ref_storage = Arc::new(RefStorage::new());
    //     let mut storage = ByteStorage::new(ref_storage.clone());
    //     let mut paths = vec![];
    //     for i in 0..1_000_000 {
    //         let path = PathBuf::from_str(format!("{}{}", path, i).as_str());
    //         paths.push(path.unwrap());
    //     }
    //     storage.extend(paths.clone());
    //     
    //     let mut path_iter = paths.iter();
    //     while let Some(slice) = ref_storage.slice(10, 0) {
    //         for item in slice.data {
    //             let Some(next) = path_iter.next() else { assert!(false); return; };
    //             assert_eq!(item.data, next.as_os_str().as_bytes());
    //         }
    //     }
    // }
}
