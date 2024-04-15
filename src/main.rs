#![allow(unused_variables, dead_code)]

use std::{env, io::{self, stderr}, sync::{atomic::{AtomicBool, AtomicU32, Ordering}, Arc, Mutex}, thread::{self, available_parallelism, JoinHandle}, time::Duration};
use std::io::Write;

use arena::Arena;
use byte_storage::FilePath;
use crossbeam::channel::{unbounded, Receiver, Sender};
use fuzzy_match::FuzzyMatcher;

use crossterm::{cursor, event, execute, queue, style::Print, terminal::{self, Clear, EnterAlternateScreen, LeaveAlternateScreen}, ExecutableCommand, QueueableCommand};
use crossterm::style::Stylize;

mod fuzzy_match;
mod byte_storage;
mod walker;
mod arena;


static WALKER_THREADS: AtomicU32 = AtomicU32::new(0);
static SHOULD_QUIT: AtomicBool = AtomicBool::new(false);
static SEARCH_WORKERS: AtomicU32 = AtomicU32::new(0);

struct SearchWorker {
    thread_id: usize,
    receiver: Receiver<String>,
    collector: Arc<Mutex<Collector>>,
    data: Arc<parking_lot::RwLock<Arena<FilePath>>>,
    tag: i32,
    current_min: i32,
    walking_is_done: bool,
    matcher: FuzzyMatcher,
    input: Option<String>,
}

impl SearchWorker {
    fn new(thread_id: usize, collector: Arc<Mutex<Collector>>, receiver: Receiver<String>, data: Arc<parking_lot::RwLock<Arena<FilePath>>>) -> Self { 
        Self { 
            thread_id,
            receiver,
            collector,
            data,
            tag: 0,
            current_min: 0,
            walking_is_done: false,
            matcher: FuzzyMatcher::new(),
            input: None,
        } 
    }

    fn run(mut self) {
        'run_loop: while !SHOULD_QUIT.load(Ordering::Relaxed) {
            if let Some(input) = self.input.clone() {
                self.current_min = 0;
                let data = self.data.read();
                let slices = data.read_only_view();
                drop(data);

                for slice in slices {
                    for chunk in slice.chunks(50) {
                        let mut points = chunk
                            .iter()
                            .map(|item| (self.matcher.smith_waterman(input.as_bytes(), *item), item))
                            .filter(|(point, _)| *point > self.current_min)
                            .collect::<Vec<_>>();

                        points.sort_unstable_by(|(p1, _), (p2, _)| p2.cmp(p1));
                        let (points, paths): (Vec<i32>, Vec<&FilePath>) = points.iter().map(|p| *p).unzip();

                        let mut collector = self.collector.lock().unwrap();
                        self.current_min = collector.update(points, paths, self.tag);
                        drop(collector);

                        if SHOULD_QUIT.load(Ordering::Relaxed) {
                            break 'run_loop
                        }
                        if self.has_new_input() {
                            continue 'run_loop
                        }
                    }
                }

                if WALKER_THREADS.load(Ordering::Relaxed) == 0 {
                    self.walking_is_done = true;
                }
                self.input = None;
            } else {
                thread::sleep(Duration::from_millis(1));
                self.has_new_input();
            }
        }

        SEARCH_WORKERS.fetch_sub(1, Ordering::Relaxed);
    }

    fn has_new_input(&mut self) -> bool {
        let mut ret = false;
        while let Ok(new_input) = self.receiver.try_recv() {
            self.tag += 1;
            self.input = Some(new_input);
            ret = true;
        }

        ret
    }
}


struct Searcher {
    handles: Vec<JoinHandle<()>>,
    collector: Arc<Mutex<Collector>>,
    work_order_senders: Vec<Sender<String>>,
    storages: Vec<Arc<parking_lot::RwLock<Arena<FilePath>>>>,
    input: String,
}

impl Searcher {
    pub fn new(storages: Vec<Arc<parking_lot::RwLock<Arena<FilePath>>>>, collector: Arc<Mutex<Collector>>) -> Self {
        let p_level = available_parallelism().unwrap().get();
        let mut handles = Vec::new();
        let mut work_order_senders = Vec::new();

        for (thread_id, storage) in storages.iter().enumerate() {
            let (wo_sender, wo_receiver) = unbounded();
            work_order_senders.push(wo_sender);
            let worker = SearchWorker::new(
                thread_id,
                collector.clone(),
                wo_receiver,
                storage.clone(),
            );
            let handle = thread::spawn(move || {
                worker.run();
            });
            handles.push(handle);
        }

        SEARCH_WORKERS.store(p_level as u32, Ordering::Relaxed);
        Searcher {
            handles,
            work_order_senders,
            collector,
            storages,
            input: String::new(),
        }
    }

    fn search(&mut self, input: String) {
        let mut collector = self.collector.lock().unwrap();
        collector.clear();
        drop(collector);

        for sender in self.work_order_senders.iter() {
            sender.send(input.clone()).unwrap();
        }
    }

    fn terminate(self) {
        for handle in self.handles {
            handle.join().unwrap();
        }
    }
}


struct Collector {
    data: Vec<FilePath>,
    points: Vec<i32>,
    capacity: usize,
    current_min: u32,
    tag: i32, 
}

impl Collector {
    fn new(capacity: usize) -> Self {
        Self {
            data: Vec::with_capacity(capacity),
            points: Vec::with_capacity(capacity),
            capacity,
            current_min: 0,
            tag: 0,
        }
    }

    fn clear(&mut self) {
        self.tag += 1;
        self.points.clear();
        self.data.clear();
        self.current_min = 0;
    }

    fn update(&mut self, points: Vec<i32>, items: Vec<&FilePath>, tag: i32) -> i32 {
        if tag != self.tag {
            return -1;
        }
        let mut new_points = Vec::with_capacity(self.capacity);
        let mut new_items = Vec::with_capacity(self.capacity);
        let mut my_index = 0;
        let mut your_index = 0;
        let mut new_index = 0;
        while my_index < self.points.len() && your_index < points.len() && new_index < self.capacity {
            if self.points[my_index] < points[your_index] {
                new_points.push(points[your_index]);
                new_items.push(items[your_index].clone());
                your_index += 1;
            } else {
                new_points.push(self.points[my_index]);
                new_items.push(self.data[my_index].clone());
                my_index += 1;
            }
            new_index += 1;
        }

        while my_index < self.points.len() && new_index < self.capacity {
            new_points.push(self.points[my_index]);
            new_items.push(self.data[my_index].clone());
            my_index += 1; new_index += 1;
        }

        while your_index < points.len() && new_index < self.capacity {
            new_points.push(points[your_index]);
            new_items.push(items[your_index].clone());
            your_index += 1; new_index += 1;
        }

        for i in 0..new_points.len() {
            if i < self.points.len() {
                self.points[i] = new_points[i];
                self.data[i] = new_items[i].clone();
            } else {
                self.points.push(new_points[i]);
                self.data.push(new_items[i].clone());
            }
        }

        if self.points.len() == self.capacity {
            return *self.points.last().unwrap()
        }
        
        -1
    }
}


fn main() -> io::Result<()> {
    // let loc = env::args().nth(1).unwrap_or(".".to_owned());
    let mut index_all = false;
    let mut cd_path = false;
    let mut loc = ".".to_owned();
    for arg in env::args().skip(1) {
        if arg.starts_with("--") {
            match &*arg {
                "--index-all" => index_all = true,
                "--cd-path" => cd_path = true,
                _ => (),
            }
        } else {
            loc = arg;
        }
    }

    let walker = walker::Walker::run(loc);
    let (cols, rows) = terminal::size().unwrap();

    let top_results = Arc::new(Mutex::new(Collector::new(15)));
    let mut searcher = Searcher::new(walker.refs.clone(), top_results.clone());

    let mut stderr = stderr();
    terminal::enable_raw_mode().unwrap();
    execute!(stderr, EnterAlternateScreen).unwrap();

    stderr.queue(cursor::MoveTo(0, 0)).unwrap();
    stderr.flush().unwrap();
    let mut buffer = String::new();
    let mut selection_index = -1;

    let mut final_print: Option<FilePath> = None;
    let mut cursor_pos = 0;
    let mut walking_done = false;
    let mut items = 0;

    'mainloop: loop {
        if index_all && WALKER_THREADS.load(Ordering::Relaxed) == 0 {
            // final_print = Some(format!("final items: {}", refs.len()));
            break 'mainloop
        }

        let (cols, rows) = terminal::size().unwrap();

        if event::poll(Duration::ZERO).unwrap() { 
            match event::read().unwrap() {
                event::Event::Key(key_event) if key_event.kind == event::KeyEventKind::Press => {
                    match key_event.code {
                        event::KeyCode::Char(ch) => {
                            stderr.queue(terminal::Clear(terminal::ClearType::All)).unwrap();
                            if key_event.modifiers.contains(event::KeyModifiers::CONTROL) {
                                match ch {
                                    'c' => {
                                        SHOULD_QUIT.store(true, Ordering::Relaxed);
                                        break;
                                    }
                                    'n' => {
                                        selection_index += 1;
                                    }
                                    'p' => {
                                        selection_index = (selection_index - 1).max(-1);
                                    }
                                    _ => (),
                                }
                            } else {
                                buffer.push(ch);
                                searcher.search(buffer.clone());
                                cursor_pos += 1;
                            }

                        }
                        event::KeyCode::Backspace => { 
                            if key_event.modifiers.contains(event::KeyModifiers::CONTROL) {
                                buffer.clear()
                            }
                            if cursor_pos > 0 {
                                buffer.remove(cursor_pos - 1);
                                cursor_pos -= 1;
                                if !buffer.is_empty() {
                                    searcher.search(buffer.clone());
                                }

                                stderr.queue(terminal::Clear(terminal::ClearType::All)).unwrap();
                            }
                        }
                        event::KeyCode::Enter => {
                            if selection_index > -1 {
                                let top_results = top_results.lock().unwrap();
                                let item = top_results.data[selection_index as usize].clone();
                                drop(top_results);

                                final_print = Some(item);
                            }
                            break 'mainloop;
                        }
                        event::KeyCode::Left => {
                            if cursor_pos > 0 {
                                cursor_pos -= 1;
                            }
                        }
                        event::KeyCode::Right => {
                            if cursor_pos < buffer.len() {
                                cursor_pos += 1;
                            }
                        }
                        _ => (),
                    }
                }
                _ => (),
            }
        }

        if !walking_done {
            items = walker.refs.iter().map(|r| r.read().len()).sum();
        }
        if !walking_done && WALKER_THREADS.load(Ordering::Relaxed) == 0 {
            walking_done = true;
        }

        queue!(
            stderr,
            cursor::MoveTo(0, 0),
            Print(format!("{}", buffer)),
            cursor::MoveTo(0, 1),
            Print(format!("items: {}, walker threads: {}, search workers: {}", items, WALKER_THREADS.load(Ordering::Relaxed), SEARCH_WORKERS.load(Ordering::Relaxed))),
            cursor::MoveTo(0, 2),
        ).unwrap();
        stderr.flush().unwrap();

        if !buffer.is_empty() {
            let top_results = top_results.lock().unwrap();
            let items = top_results.data.clone();
            let points = top_results.points.clone();
            drop(top_results);

            for (index, item) in items.iter().enumerate() {
                stderr.queue(cursor::MoveTo(0, 2 + index as u16)).unwrap();
                if selection_index == index as i32 {
                    write!(stderr, "{}{}{}", item.name().white().on_black(), " --> ".white().on_black(), item.path().white().on_black()).unwrap();
                } else {
                    write!(stderr, "{}{}{}", item.name(), " --> ", item.path()).unwrap();
                }
            }
        }

        stderr.queue(cursor::MoveTo(cursor_pos as u16, 0)).unwrap();
        stderr.flush().unwrap();
        thread::sleep(Duration::from_millis(1000 / 30))
    }
    
    SHOULD_QUIT.store(true, Ordering::Relaxed);
    stderr.execute(Clear(terminal::ClearType::All)).unwrap();
    execute!(stderr, LeaveAlternateScreen).unwrap();
    terminal::disable_raw_mode().unwrap();
    searcher.terminate();

    if let Some(item) = final_print {
        if cd_path {
            println!("{}", item.containing_folder());
        } else {
            println!("{}", item.to_string());
        }
    } else {
        println!("boo done :3");
    }

    Ok(())
}
