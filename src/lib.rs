#![feature(proc_macro_hygiene)]
#![feature(asm)]
#![allow(dead_code)]
#![no_std]
// #![feature(alloc)]

mod logger;

#[macro_use]
extern crate alloc;
use alloc::vec::Vec;

#[macro_use]
extern crate nn;

#[macro_use]
extern crate log;

use nn::static_c_str as c;

static mut ALLOC_MEM: *mut libc::c_void = 0 as _;
const HEAP_SIZE: usize = 0x800000;
const GRAPHICS_MEM_SIZE: usize = 0x400000;
static mut ALLOCATOR: nn::mem::StandardAllocator = nn::mem::StandardAllocator::default();
static mut LOGGER: logger::FileLogger = logger::FileLogger::new("sd:/output.log");

struct GlobalAllocator;
unsafe impl core::alloc::GlobalAlloc for GlobalAllocator {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 { malloc(layout.size()) as _ }
    unsafe fn dealloc(&self, ptr: *mut u8, _layout: core::alloc::Layout) { free(ptr as _) }
}

#[global_allocator]
static A: GlobalAllocator = GlobalAllocator;

extern "C" fn fs_alloc(size: usize) -> *mut libc::c_void {
    unsafe {
        malloc(size)
    }
}

extern "C" fn fs_dealloc(addr: *mut libc::c_void, _size: usize) {
    unsafe {
        free(addr)
    }
}

#[no_mangle]
unsafe extern "C" fn malloc(size: usize) -> *mut libc::c_void {
    ALLOCATOR.alloc(size)
}

#[no_mangle]
unsafe extern "C" fn free(addr: *mut libc::c_void) {
    if !addr.is_null() {
        ALLOCATOR.free(addr)
    }
}

#[no_mangle]
unsafe extern "C" fn calloc(num: usize, size: usize) -> *mut libc::c_void {
    let sum = num * size;
    let addr = malloc(sum);
    if !addr.is_null() {
        libc::memset(addr, 0, sum);
    }
    addr
}

#[no_mangle]
unsafe extern "C" fn realloc(addr: *mut libc::c_void, size: usize) -> *mut libc::c_void {
    ALLOCATOR.realloc(addr, size)
}

#[no_mangle]
unsafe extern "C" fn aligned_alloc(align: usize, size: usize) -> *mut libc::c_void {
    ALLOCATOR.alloc_aligned(size, align)
}

#[allow(non_snake_case)]
#[export_name = "nninitStartup"]
pub extern "C" fn nninitStartup() {
    unsafe {
        nn::os::set_heap_size(HEAP_SIZE);
        ALLOC_MEM = nn::os::alloc_from_heap(HEAP_SIZE).unwrap();
        ALLOCATOR.init(ALLOC_MEM, 0x800000);
    }    
}

unsafe fn walk_switch_dir() {
    let handle = nn::fs::open_directory("sd:/switch", nn::fs::OpenDirectoryMode::ALL).unwrap();
    let entry_count = nn::fs::get_directory_entry_count(handle).unwrap();
    let mut entries = Vec::<nn::fs::DirectoryEntry>::new();
    entries.resize(entry_count as usize, core::mem::zeroed());
    let entries_read = nn::fs::read_directory_entries(&mut entries, handle).unwrap();
    if entry_count != entries_read {
        return;
    }
    nn::fs::close_directory(handle);
    let handle = open_or_create("sd:/switch.txt");
    let mut offset = 0;
    for entry in entries.iter() {
        let len = libc::strlen(entry.name.as_ptr());
        nn::fs::write_file(handle, offset, entry.name.as_ptr() as _, len, nn::fs::WriteOptions::FLUSH).unwrap();
        offset += len as isize;
        nn::fs::write_file(handle, offset, &0xAu8 as *const u8 as _, 1, nn::fs::WriteOptions::FLUSH).unwrap();
        offset += 1;
    }
    nn::fs::close_file(handle);
}

fn open_or_create(path: &'static str) -> nn::fs::FileHandle {
    match nn::fs::open_file(path, nn::fs::OpenMode::WRITE | nn::fs::OpenMode::ALLOW_APPEND) {
        Ok(handle) => {
            // nn::fs::resize_file(handle, 0).unwrap();
            handle
        },
        Err(_) => {
            nn::fs::create_file(path, 0).unwrap();
            nn::fs::open_file(path, nn::fs::OpenMode::WRITE | nn::fs::OpenMode::ALLOW_APPEND).unwrap()
        }
    }
}

extern "C" fn thread_func(_: *mut libc::c_void) {
    let mut counter = 0;
    loop {
        nn::os::Thread::sleep(nn::TimeSpan::from_secs(1));
        info!("This is a sample message {}", counter);
        counter += 1;
    }
}

static mut DISPLAY: nn::vi::Display = nn::vi::Display::uninit();
static mut LAYER: nn::vi::Layer = nn::vi::Layer::uninit();
static mut NATIVE_HANDLE: nn::vi::NativeWindowHandle = nn::vi::NativeWindowHandle::uninit();

fn init_graphics() {
    extern "C" fn gfx_alloc(size: usize, align: usize, _: *mut libc::c_void) -> *mut libc::c_void {
        unsafe {
            aligned_alloc(align, size)
        }
    }

    extern "C" fn gfx_dealloc(addr: *mut libc::c_void, _: *mut libc::c_void) {
        unsafe {
            free(addr)
        }
    }

    extern "C" fn gfx_realloc(addr: *mut libc::c_void, new_size: usize, _: *mut libc::c_void) -> *mut libc::c_void {
        unsafe {
            realloc(addr, new_size)
        }
    }

    nv::set_allocators(gfx_alloc, gfx_dealloc, gfx_realloc);
    unsafe {
        nv::init(malloc(GRAPHICS_MEM_SIZE), GRAPHICS_MEM_SIZE);
    }
    
    nn::vi::init();
    unsafe {
        DISPLAY = nn::vi::Display::open_default().unwrap();
        LAYER = nn::vi::Layer::new(DISPLAY).unwrap();
        NATIVE_HANDLE = LAYER.native_handle().unwrap();
    }
    
    nvn::init();
}

#[allow(non_snake_case)]
#[export_name = "nnMain"]
pub extern "C" fn nnMain() {
    nn::fs::set_alloc_funcs(fs_alloc, fs_dealloc);
    nn::fs::mount_sd_card("sd").unwrap();
    // nvn::graphics_init();
    init_graphics();
    let args = nn::os::get_host_args();
    unsafe {
        LOGGER.init();
        log::set_logger(&LOGGER).map(|()| log::set_max_level(log::LevelFilter::Info)).unwrap();
        info!("{:?}", args);
        let stack = aligned_alloc(0x1000, 0x8000);
        let mut thread = nn::os::Thread::new_on_core(thread_func, 0 as _, stack, 0x8000, 16, 0).unwrap();
        thread.start();

        walk_switch_dir();
        info!("{:#x}", nvn::global_device().get_proc("nvnDeviceInitialize\0".as_ptr() as _) as u64);

        loop {
            nn::os::Thread::yield_now();
        }
    }
}