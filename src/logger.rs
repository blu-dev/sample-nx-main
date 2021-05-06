// Very simple logger implementation
// This logger DOES NOT buffer output, it flushes when it writes
// For this reason, writing larger amounts of text is probably best
// or else you will receive decent slowdown

// An implementation similar to skyline's TcpLogger is planned for the future
// with a potential on-screen console as well

use log::{Record, Level, Metadata};

pub struct FileLogger {
    path: &'static str,
    handle: nn::fs::FileHandle,
    offset: *mut isize // Log trait requires static ref so I need to make this a ptr
}

unsafe impl Send for FileLogger {}
unsafe impl Sync for FileLogger {}

impl FileLogger {
    pub const fn new(path: &'static str) -> Self {
        FileLogger {
            path,
            handle: nn::fs::FileHandle(0),
            offset: 0 as *mut isize
        }
    }

    pub fn init(&mut self) {
        if self.handle.0 == 0 {
            self.handle = match nn::fs::open_file(self.path, nn::fs::OpenMode::WRITE | nn::fs::OpenMode::ALLOW_APPEND) {
                Ok(v) => {
                    if self.offset.is_null() {
                        nn::fs::resize_file(v, 0).unwrap();
                    }
                    v
                },
                Err(_) => {
                    nn::fs::create_file(self.path, 0).unwrap();
                    nn::fs::open_file(self.path, nn::fs::OpenMode::WRITE | nn::fs::OpenMode::ALLOW_APPEND).unwrap()
                }
            };
            unsafe {
                if self.offset.is_null() {
                    self.offset = super::calloc(1, core::mem::size_of::<isize>()) as _;
                }
            }
        }
    }

    pub fn close(&mut self) {
        nn::fs::close_file(self.handle);
        self.handle = nn::fs::FileHandle(0);
    }
}

impl log::Log for FileLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Info
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) && self.handle.0 != 0 {
            let msg = format!("[{}]: {}\n", record.level(), record.args());
            unsafe {
                nn::fs::write_file(self.handle, *self.offset, msg.as_ptr() as _, msg.len(), nn::fs::WriteOptions::FLUSH);
                *self.offset += msg.len() as isize;
            }
        }
    }

    fn flush(&self) {}
}