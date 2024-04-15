use std::{mem, ptr::slice_from_raw_parts};

const ALLOC_SIZE: usize = 1_000_000;

pub struct Arena<T: Clone> {
    current: Vec<T>,
    full: Vec<Vec<T>>,
}

impl<T: Clone> Arena<T> {
    pub fn new() -> Self {
        Self {
            current: Vec::with_capacity(ALLOC_SIZE),
            full: Vec::new(),
        }
    }

    pub fn push(&mut self, value: T) {
        if self.current.len() >= ALLOC_SIZE {
            let old = mem::replace(&mut self.current, Vec::with_capacity(ALLOC_SIZE));
            self.full.push(old);
        }
        self.current.push(value);
    }

    pub fn extend(&mut self, data: &[T]) {
        if self.current.len() + data.len() > ALLOC_SIZE {
            let old = mem::replace(&mut self.current, Vec::with_capacity(ALLOC_SIZE.max(data.len())));
            self.full.push(old);
        }
        self.current.extend_from_slice(data);
    }

    pub fn extend_and_get(&mut self, data: &[T]) -> &'static [T] {
        if self.current.len() + data.len() > ALLOC_SIZE {
            let old = mem::replace(&mut self.current, Vec::with_capacity(ALLOC_SIZE.max(data.len())));
            self.full.push(old);
        }

        let start = self.current.len();
        let len = data.len();
        self.current.extend_from_slice(data);

        unsafe { &*slice_from_raw_parts(&self.current[start], len) }
    }

    pub fn read_only_view(&self) -> Vec<&'static [T]> {
        let mut slices = Vec::new();

        let slice = unsafe { &*slice_from_raw_parts(&self.current[0], self.current.len()) };
        slices.push(slice);

        for slice in &self.full {
            if slice.is_empty() { break; }
            let raw_slice = unsafe { &*slice_from_raw_parts(&slice[0], slice.len()) };
            slices.push(raw_slice);
        }
        
        slices
    }

    pub fn len(&self) -> usize {
        self.current.len() + self.full.iter().map(|v| v.len()).sum::<usize>()
    }
}

