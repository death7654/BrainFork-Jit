use std::mem;
use windows_sys::Win32::System::Memory::{VirtualAlloc, VirtualProtect, MEM_COMMIT, MEM_RESERVE, PAGE_READWRITE, PAGE_EXECUTE_READ};
use std::os::raw::c_void;
const PAGE_SIZE: usize = 4096;
struct JitMemory
{
    code_buffer: *mut u8,
}

impl JitMemory {
    pub fn new(num_pages: usize) -> Self
    {
        let code_buffer: * mut u8;
        unsafe 
        {
            let size: usize = num_pages * PAGE_SIZE;
            let allocation_type = MEM_COMMIT | MEM_RESERVE;
            let mut _contents: *mut c_void = VirtualAlloc(std::ptr::null_mut(), size, allocation_type, PAGE_READWRITE);
            let dest = _contents as *mut u8;

            if _contents.is_null() {
                panic!("Failed to allocate executable memory!");
            }

            // let ret: [u8; 4] = [0xC3, 0x00, 0x00, 0x00]; standard ret
            let ret: [u8; 9] = [0xB8, 0x2A, 0x00, 0x00, 0x00, 0xC3, 0x00, 0x00, 0x00];
            std::ptr::copy_nonoverlapping(ret.as_ptr(), dest, ret.len());

            let mut old_protect = 0u32;
            let success = VirtualProtect(_contents, size, PAGE_EXECUTE_READ, &mut old_protect);

            if success == 0 {
                panic!("Failed to set memory to PAGE_EXECUTE_READ!");
            }
            code_buffer = mem::transmute(_contents);
        }
        JitMemory { code_buffer }
    }

    pub fn execute(self) -> i32
    {
        unsafe{
            let func: extern "C" fn() -> i32 = mem::transmute(self.code_buffer);
            func()
        }
    }
}

fn main() {
    println!("Hello, world!");
    let jit = JitMemory::new(1);
    let result = jit.execute();
    println!("Output from Jit {}", result);
}
