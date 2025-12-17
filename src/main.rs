/*Some help to code this project:
* https://www.reddit.com/r/embedded/comments/1myrsqj/i_wrote_a_minimal_embedded_fat32_file_system/?tl=fr
* https://www.reddit.com/r/rust/comments/9eyc21/noob_what_exactly_is_no_std_and_why_is_it_so/?tl=fr
* https://docs.rs/hadris-fat/latest/hadris_fat/
* https://crates.io/crates/fat32rs
* and others github projects...
*
* Some help to debug:
* Github Copilot
* Google Gemini
*/

#![no_std]
#![no_main]

extern crate alloc;
use alloc::vec::Vec;
use alloc::string::String;
use alloc::format;
use core::ffi::c_void;
use core::alloc::{GlobalAlloc, Layout};
use core::panic::PanicInfo;
mod fat32;
use crate::fat32::volume::Fat32Volume;

#[link(name = "c")]
extern "C" {}

#[no_mangle]
pub unsafe extern "C" fn memcpy(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    let mut i = 0;
    while i < n {
        *dest.add(i) = *src.add(i);
        i += 1;
    }
    dest
}

#[no_mangle]
pub unsafe extern "C" fn memset(s: *mut u8, c: i32, n: usize) -> *mut u8 {
    let mut i = 0;
    while i < n {
        *s.add(i) = c as u8;
        i += 1;
    }
    s
}

#[no_mangle]
pub unsafe extern "C" fn memcmp(s1: *const u8, s2: *const u8, n: usize) -> i32 {
    let mut i = 0;
    while i < n {
        let a = *s1.add(i);
        let b = *s2.add(i);
        if a != b { return a as i32 - b as i32; }
        i += 1;
    }
    0
}

#[no_mangle]
pub extern "C" fn rust_eh_personality() {}

#[no_mangle]
pub extern "C" fn _Unwind_Resume() -> ! {
    loop {}
}

struct LibcAllocator;

unsafe impl GlobalAlloc for LibcAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        libc::malloc(layout.size()) as *mut u8
    }
    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        libc::free(ptr as *mut c_void);
    }
}

#[global_allocator]
static ALLOCATOR: LibcAllocator = LibcAllocator;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    let msg = "!!! KERNEL PANIC !!!\n";
    unsafe { 
        libc::write(1, msg.as_ptr() as *const c_void, msg.len());
        libc::exit(1) 
    };
}

fn sys_print(s: &str) {
    unsafe {
        libc::write(1, s.as_ptr() as *const c_void, s.len());
        libc::write(1, "\n".as_ptr() as *const c_void, 1);
    }
}

fn sys_print_raw(s: &str) {
    unsafe { libc::write(1, s.as_ptr() as *const c_void, s.len()); }
}

fn sys_read_line() -> String {
    let mut buffer = Vec::new();
    let mut c: [u8; 1] = [0];
    loop {
        let n = unsafe { libc::read(0, c.as_mut_ptr() as *mut c_void, 1) };
        if n <= 0 { break; }
        if c[0] == b'\n' { break; }
        buffer.push(c[0]);
    }
    String::from_utf8_lossy(&buffer).trim().into()
}

fn sys_open_rw(path: &str) -> i32 {
    let path_c = format!("{}\0", path);
    unsafe { libc::open(path_c.as_ptr() as *const i8, libc::O_RDWR) }
}

fn sys_read_all(fd: i32) -> Vec<u8> {
    unsafe {
        let size = libc::lseek(fd, 0, libc::SEEK_END);
        libc::lseek(fd, 0, libc::SEEK_SET); 
        if size <= 0 { return Vec::new(); }
        
        let mut buffer = Vec::with_capacity(size as usize);
        buffer.set_len(size as usize); 
        libc::read(fd, buffer.as_mut_ptr() as *mut c_void, size as usize);
        buffer
    }
}

fn sys_write_all(fd: i32, data: &[u8]) {
    unsafe {
        libc::lseek(fd, 0, libc::SEEK_SET);
        libc::write(fd, data.as_ptr() as *const c_void, data.len());
    }
}

#[no_mangle]
pub extern "C" fn main(_argc: isize, _argv: *const *const u8) -> isize {
    let img_path = "fat32.img";
    
    sys_print("--- FAT32 Shell (100% No-Std / LibC) ---");
    sys_print_raw("Opening image... ");
    
    let fd = sys_open_rw(img_path);
    if fd < 0 {
        sys_print("Error: Cannot open fat32.img");
        return 1;
    }
    sys_print("OK.");

    let mut disk_memory = sys_read_all(fd);
    if disk_memory.is_empty() {
        sys_print("Error: Empty image.");
        return 1;
    }

    loop {
        sys_print_raw("> ");
        let input = sys_read_line();
        if input.is_empty() { continue; }
        
        let mut parts = input.split(' ');
        let command = parts.next().unwrap_or("");
        let arg1 = parts.next();
        let arg_rest = if let Some(a1) = arg1 {
             let start = command.len() + 1 + a1.len();
             if start < input.len() { Some(&input[start..]) } else { None }
        } else { None };

        let mut volume = Fat32Volume::new(&mut disk_memory);

        match command {
            "exit" | "quit" => break,
            "info" => sys_print(&volume.get_info()),
            "ls" => {
                let files = volume.list_root();
                for f in files { sys_print(&f); }
            }
            "cat" => {
                if let Some(filename) = arg1 {
                    match volume.read_file(filename) {
                        Ok(content) => {
                            let s = String::from_utf8_lossy(&content);
                            sys_print(&s);
                        },
                        Err(e) => sys_print(e),
                    }
                } else { sys_print("Usage: cat <filename>"); }
            }
            "touch" => {
                if let Some(filename) = arg1 {
                    let content = arg_rest.unwrap_or("").trim();
                    match volume.create_file(filename, content.as_bytes()) {
                        Ok(_) => sys_print("File created."),
                        Err(e) => sys_print(e),
                    }
                } else { sys_print("Usage: touch <filename> <text>"); }
            }
            _ => sys_print("Unknown command."),
        }
    }

    sys_print("Saving...");
    sys_write_all(fd, &disk_memory);
    unsafe { libc::close(fd); }
    sys_print("Bye.");
    0
}