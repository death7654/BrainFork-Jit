use std::io::{self, Read, Write};
use std::mem;
use std::os::raw::c_void;
use windows_sys::Win32::System::Memory::{
    MEM_COMMIT, MEM_RESERVE, PAGE_EXECUTE_READ, PAGE_READWRITE, VirtualAlloc, VirtualProtect,
};

const PAGE_SIZE: usize = 4096;
const CODE_PAGES: usize = 4;
const TAPE_PAGES: usize = 8;
const CODE_BUFFER_SIZE: usize = CODE_PAGES * PAGE_SIZE;
const TAPE_BUFFER_SIZE: usize = TAPE_PAGES * PAGE_SIZE;

struct BrainFork {
    code_buffer: *mut u8,
    tape_buffer: *mut u8,
}

impl BrainFork {
    pub fn new() -> Self {
        let code_buffer: *mut u8;
        let tape_buffer: *mut u8;

        let allocation_type = MEM_COMMIT | MEM_RESERVE;
        unsafe {
            // allocate code buffers
            let mut _code_contents: *mut c_void = VirtualAlloc(
                std::ptr::null_mut(),
                CODE_BUFFER_SIZE,
                allocation_type,
                PAGE_READWRITE,
            );
            let dest = _code_contents as *mut u8;

            if _code_contents.is_null() {
                panic!("Failed to allocate executable memory!");
            }

            // default values for protection, 0xC3 = RET
            let ret: [u8; 4] = [0xC3, 0x00, 0x00, 0x00];
            std::ptr::copy_nonoverlapping(ret.as_ptr(), dest, ret.len());

            // convert into *mut u8
            code_buffer = mem::transmute(_code_contents);

            // allocate tape buffers
            let _tape_contents: *mut c_void = VirtualAlloc(
                std::ptr::null_mut(),
                TAPE_BUFFER_SIZE,
                allocation_type,
                PAGE_READWRITE,
            );

            if _tape_contents.is_null() {
                panic!("Failed to allocate tape memory!");
            }

            // convert into *mut u8
            tape_buffer = mem::transmute(_tape_contents);
        }

        BrainFork {
            code_buffer,
            tape_buffer,
        }
    }

    pub fn compile(&mut self, source: &str) {
        let mut compiled: Vec<u8> = Vec::new();
        let mut jump: Vec<usize> = Vec::new();
        for c in source.chars() {
            match c {
                // inc byte ptr [rcx], Increment cell value at tape pointer
                '+' => compiled.extend_from_slice(&[0xFE, 0x01]),

                // dec byte ptr [rcx], Decrement cell value at tape pointer
                '-' => compiled.extend_from_slice(&[0xFE, 0x09]),

                // inc rcx, Move tape pointer right
                '>' => compiled.extend_from_slice(&[0x48, 0xFF, 0xC1]),

                // dec rcx, Move tape pointer left
                '<' => compiled.extend_from_slice(&[0x48, 0xFF, 0xC9]),

                // loop
                '[' => {
                    // cmp byte ptr [rcx], 0
                    compiled.extend_from_slice(&[0x80, 0x39, 0x00]);

                    // je placeholder
                    compiled.extend_from_slice(&[0x74, 0x00]);

                    // Push index of the 0x00 placeholder byte
                    jump.push(compiled.len() - 1);
                }

                // loop end
                ']' => {
                    let placeholder_idx = jump
                        .pop()
                        .expect("Reached end of loop ']' but no loop was started!");

                    // Calculate backward offset to cmp
                    let target_cmp = placeholder_idx - 4;
                    let next_ip_after_jmp = compiled.len() + 2; // includes 0xEB and offset byte
                    let backward_offset =
                        (target_cmp as isize - next_ip_after_jmp as isize) as i8 as u8;

                    // Emit unconditional backward jump
                    compiled.extend_from_slice(&[0xEB, backward_offset]);

                    // Backpatch the forward jump placeholder
                    let forward_offset = (compiled.len() - (placeholder_idx + 1)) as u8;
                    compiled[placeholder_idx] = forward_offset;
                }
                '.' => {
                    // push rcx
                    compiled.push(0x51);

                    // sub rsp, 32
                    compiled.extend_from_slice(&[0x48, 0x83, 0xEC, 0x20]);

                    // movzx ecx, byte ptr [rcx]
                    compiled.extend_from_slice(&[0x0F, 0xB6, 0x09]);

                    // mov rax, <print_char address>

                    compiled.extend_from_slice(&[0x48, 0xB8]);
                    let fn_addr = print_char as *const () as usize as u64;

                    compiled.extend_from_slice(&fn_addr.to_le_bytes());

                    // call rax
                    compiled.extend_from_slice(&[0xFF, 0xD0]);

                    // add rsp, 32
                    compiled.extend_from_slice(&[0x48, 0x83, 0xC4, 0x20]);

                    // pop rcx
                    compiled.push(0x59);
                }
                ',' => {
                    // push rcx
                    compiled.push(0x51);

                    // sub rsp, 32
                    compiled.extend_from_slice(&[0x48, 0x83, 0xEC, 0x20]);

                    // mov rax, <read_char address>
                    compiled.extend_from_slice(&[0x48, 0xB8]);
                    let fn_addr = read_char as usize as u64;
                    compiled.extend_from_slice(&fn_addr.to_le_bytes());

                    // call rax
                    compiled.extend_from_slice(&[0xFF, 0xD0]);

                    // add rsp, 32
                    compiled.extend_from_slice(&[0x48, 0x83, 0xC4, 0x20]);

                    // pop rcx
                    compiled.push(0x59);

                    // mov byte ptr [rcx], al
                    compiled.extend_from_slice(&[0x88, 0x01]);
                }
                // Ignore non-Brainfork characters
                _ => {}
            }
        }

        if !jump.is_empty() {
            panic!("Unmatched opening bracket '[' detected!");
        }

        compiled.push(0xC3);

        unsafe {
            // set code block to write
            let mut old_protect = 0u32;
            let success = VirtualProtect(
                self.code_buffer as *const c_void,
                CODE_PAGES * PAGE_SIZE,
                PAGE_READWRITE,
                &mut old_protect,
            );
            if success == 0 {
                panic!("Failed to set memory to PAGE_READ_WRITE!");
            }

            std::ptr::copy_nonoverlapping(compiled.as_ptr(), self.code_buffer, compiled.len());

            // set code block to execute
            let mut old_protect = 0u32;
            let success = VirtualProtect(
                self.code_buffer as *const c_void,
                CODE_PAGES * PAGE_SIZE,
                PAGE_EXECUTE_READ,
                &mut old_protect,
            );
            if success == 0 {
                panic!("Failed to set memory to PAGE_EXECUTE_READ!");
            }
        }
    }

    pub fn execute(&self) {
        unsafe {
            let func: extern "C" fn(*mut u8) = mem::transmute(self.code_buffer);
            func(self.tape_buffer);
        }
    }
}

extern "win64" fn print_char(byte: u8) {
    print!("{}", byte as char);
    let _ = io::stdout().flush();
}

extern "win64" fn read_char() -> u8 {
    let mut buffer = [0u8; 1];
    if io::stdin().read_exact(&mut buffer).is_ok() {
        buffer[0]
    } else {
        0
    }
}

fn main() {
    let mut jit = BrainFork::new();
    let code = "++++++++[>++++[>++>+++>+++>+<<<<-]>+>+>->>+[<]<-]>>.>---.+++++++..+++.>>.<-.<.+++.------.--------.>>+.>++.";
    jit.compile(code);

    println!("Running JIT compiled Brainfork...");
    jit.execute();
    println!("Done!");

    // unsafe {
    //     println!("Value in tape cell [0]: {}", *jit.tape_buffer);
    // }
}
