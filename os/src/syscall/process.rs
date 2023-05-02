//! Process management syscalls

use crate::{
    config::MAX_SYSCALL_NUM,
    task::{
        change_program_brk, exit_current_and_run_next, suspend_current_and_run_next, TaskStatus, current_user_token, current_status, current_syscall_times, current_sys_time, insert_map, remove_map
    }, timer::{get_time_us, get_time_ms}, mm::{VirtAddr, VPNRange, vaddr_mapped, MapPermission}
};

use crate::mm::translated_byte_buffer;

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

/// Task information
#[allow(dead_code)]
pub struct TaskInfo {
    /// Task status in it's life cycle
    status: TaskStatus,
    /// The numbers of syscall called by task
    syscall_times: [u32; MAX_SYSCALL_NUM],
    /// Total running time of task
    time: usize,
}

/// task exits and submit an exit code
pub fn sys_exit(_exit_code: i32) -> ! {
    trace!("kernel: sys_exit");
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
}

/// YOUR JOB: get time with second and microsecond
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TimeVal`] is splitted by two pages ?
pub fn sys_get_time(_ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
    let us = get_time_us();
    let timeval = translated_byte_buffer(current_user_token(), _ts as *const u8, core::mem::size_of::<TimeVal>());
    if timeval.len() == 1 {
        let ts = unsafe { core::mem::transmute::<*const u8, *mut TimeVal>(timeval[0].as_ptr())};
        unsafe {
            *ts = TimeVal {
                sec: us / 1_000_000,
                usec: us % 1_000_000
            };
        }
    } else if timeval.len() == 2 {
        let ts_sec = unsafe {
            core::mem::transmute::<*const u8 , &mut usize>(timeval[0].as_ptr())
        };
        let ts_usec = unsafe {
            core::mem::transmute::<*const u8, &mut usize>(timeval[1].as_ptr())
        };
        *ts_sec = us / 1_000_000;
        *ts_usec = us % 1_000_000;
    } else {
        panic!("TimeVal byte Error");
    }
    0
}

/// YOUR JOB: Finish sys_task_info to pass testcases
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TaskInfo`] is splitted by two pages ?
pub fn sys_task_info(_ti: *mut TaskInfo) -> isize {
    trace!("kernel: sys_task_info NOT IMPLEMENTED YET!");
    let status = current_status();
    let syscall_times = current_syscall_times();
    let time = get_time_ms() - current_sys_time();
    let taskinfo = translated_byte_buffer(current_user_token(), _ti as *const u8, core::mem::size_of::<TaskInfo>());
    if taskinfo.len() == 1 {
        let ti = unsafe { core::mem::transmute::<*const u8, *mut TaskInfo>(taskinfo[0].as_ptr())};
        unsafe {
            *ti = TaskInfo {
                status,
                syscall_times, 
                time
            };
        }
    } else {
        panic!("error, can't do");
    }
    0
}

// YOUR JOB: Implement mmap.
pub fn sys_mmap(_start: usize, _len: usize, _port: usize) -> isize {
    trace!("kernel: sys_mmap NOT IMPLEMENTED YET!");
    let end = _start + _len;
    let start_va = VirtAddr::from(_start);
    if (_port & !0x7 != 0) || (_port & 0x7 == 0) {
        -1
    } else if start_va.aligned(){
        let start_vpn = start_va.floor();
        let end_va = VirtAddr::from(end);
        let end_vpn = end_va.ceil();
        let vpn_range = VPNRange::new(start_vpn, end_vpn);
        for vpn in vpn_range {
            if vaddr_mapped(current_user_token(), vpn) == true {
                return -1;
            }
        }
        let mut permission = MapPermission::U;
        if (_port & 0x1) == 0x1  {
            permission |= MapPermission::R;
        }
        if (_port & 0x2) == 0x2 {
            permission |= MapPermission::W;
        }
        if (_port & 0x4) > 0 {
            permission |= MapPermission::X;
        }
        insert_map(start_va, end_va, permission);
        0
    } else {
        -1
    }
}

// YOUR JOB: Implement munmap.
pub fn sys_munmap(_start: usize, _len: usize) -> isize {
    trace!("kernel: sys_munmap NOT IMPLEMENTED YET!");
    let end = _start + _len;
    let start_va = VirtAddr::from(_start);
    if start_va.aligned(){
        let start_vpn = start_va.floor();
        let end_va = VirtAddr::from(end);
        let end_vpn = end_va.ceil();
        let vpn_range = VPNRange::new(start_vpn, end_vpn);
        for vpn in vpn_range {
            if !vaddr_mapped(current_user_token(), vpn) {
                return -1;
            }
        }
        remove_map(start_va, end_va);
        0
    } else {
        -1
    }
}
/// change data segment size
pub fn sys_sbrk(size: i32) -> isize {
    trace!("kernel: sys_sbrk");
    if let Some(old_brk) = change_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}

//https://stackoverflow.com/questions/42499049/transmuting-u8-buffer-to-struct-in-rust
