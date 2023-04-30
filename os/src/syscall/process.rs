//!
use alloc::sync::Arc;

use crate::{
    config::MAX_SYSCALL_NUM,
    fs::{open_file, OpenFlags},
    mm::{vaddr_mapped, VPNRange, VirtAddr, MapPermission, translated_str, translated_refmut, translated_byte_buffer},
    task::{
        add_task, current_task, current_user_token, exit_current_and_run_next,
        suspend_current_and_run_next, TaskStatus, current_sys_time, current_status, current_syscall_times
    }, timer::{get_time_us, get_time_ms},
};

// use crate::mm::translated_byte_buffer;

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize, }

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

pub fn sys_exit(exit_code: i32) -> ! {
    trace!("kernel:pid[{}] sys_exit", current_task().unwrap().pid.0);
    exit_current_and_run_next(exit_code);
    panic!("Unreachable in sys_exit!");
}

pub fn sys_yield() -> isize {
    //trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
}

pub fn sys_getpid() -> isize {
    trace!("kernel: sys_getpid pid:{}", current_task().unwrap().pid.0);
    current_task().unwrap().pid.0 as isize
}

pub fn sys_fork() -> isize {
    trace!("kernel:pid[{}] sys_fork", current_task().unwrap().pid.0);
    let current_task = current_task().unwrap();
    let new_task = current_task.fork();
    let new_pid = new_task.pid.0;
    // modify trap context of new_task, because it returns immediately after switching
    let trap_cx = new_task.inner_exclusive_access().get_trap_cx();
    // we do not have to move to next instruction since we have done it before
    // for child process, fork returns 0
    trap_cx.x[10] = 0;
    // add new task to scheduler
    add_task(new_task);
    new_pid as isize
}

pub fn sys_exec(path: *const u8) -> isize {
    trace!("kernel:pid[{}] sys_exec", current_task().unwrap().pid.0);
    let token = current_user_token();
    let path = translated_str(token, path);
    if let Some(app_inode) = open_file(path.as_str(), OpenFlags::RDONLY) {
        let all_data = app_inode.read_all();
        let task = current_task().unwrap();
        task.exec(all_data.as_slice());
        0
    } else {
        -1
    }
}

/// If there is not a child process whose pid is same as given, return -1.
/// Else if there is a child process but it is still running, return -2.
pub fn sys_waitpid(pid: isize, exit_code_ptr: *mut i32) -> isize {
    //trace!("kernel: sys_waitpid");
    let task = current_task().unwrap();
    // find a child process

    // ---- access current PCB exclusively
    let mut inner = task.inner_exclusive_access();
    if !inner
        .children
        .iter()
        .any(|p| pid == -1 || pid as usize == p.getpid())
    {
        return -1;
        // ---- release current PCB
    }
    let pair = inner.children.iter().enumerate().find(|(_, p)| {
        // ++++ temporarily access child PCB exclusively
        p.inner_exclusive_access().is_zombie() && (pid == -1 || pid as usize == p.getpid())
        // ++++ release child PCB
    });
    if let Some((idx, _)) = pair {
        let child = inner.children.remove(idx);
        // confirm that child will be deallocated after being removed from children list
        assert_eq!(Arc::strong_count(&child), 1);
        let found_pid = child.getpid();
        // ++++ temporarily access child PCB exclusively
        let exit_code = child.inner_exclusive_access().exit_code;
        // ++++ release child PCB
        *translated_refmut(inner.memory_set.token(), exit_code_ptr) = exit_code;
        found_pid as isize
    } else {
        -2
    }
    // ---- release current PCB automatically
}

/// YOUR JOB: get time with second and microsecond
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TimeVal`] is splitted by two pages ?
pub fn sys_get_time(ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
    let us = get_time_us();
    let mut timeval = translated_byte_buffer(
        current_user_token(),
        ts as *const u8,
        core::mem::size_of::<TimeVal>(),
    );
    let ts: *mut TimeVal = timeval[0].as_mut_ptr().cast();
    unsafe {
        *ts = TimeVal {
            sec: us / 1_000_000,
            usec: us % 1_000_000,
        };
    }
    0
}

/// YOUR JOB: Finish sys_task_info to pass testcases
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TaskInfo`] is splitted by two pages ?
pub fn sys_task_info(ti: *mut TaskInfo) -> isize {
    trace!("kernel: sys_task_info has been implement!");
    let status = current_status();
    let syscall_times = current_syscall_times();
    let time = get_time_ms() - current_sys_time();
    let mut taskinfo = translated_byte_buffer(
        current_user_token(),
        ti as *const u8,
        core::mem::size_of::<TaskInfo>(),
    );
    let ti: *mut TaskInfo = taskinfo[0].as_mut_ptr().cast();
    unsafe {
        *ti = TaskInfo {
            status,
            syscall_times,
            time,
        };
    }
    0
}

/// YOUR JOB: Implement mmap.
pub fn sys_mmap(_start: usize, _len: usize, port: usize) -> isize {
    trace!("kernel: sys_mmap NOT IMPLEMENTED YET!");
    let end = _start + _len;
    let start_va = VirtAddr::from(_start);
    if (port & !0x7 != 0) || (port & 0x7 == 0) {
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
        if (port & 0x1) == 0x1 {
            permission |= MapPermission::R;
        }
        if (port & 0x2) == 0x2 {
            permission |= MapPermission::W;
        }
        if (port & 0x4) > 0 {
            permission |= MapPermission::X;
        }
        let task = current_task().unwrap();
        task.mmap(start_va, end_va, permission);
        0
    } else {
        -1
    }
}

/// YOUR JOB: Implement munmap.
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
        let task = current_task().unwrap();
        task.munmap(start_va, end_va);
        0
    } else {
        -1
    }
}

/// change data segment size
pub fn sys_sbrk(size: i32) -> isize {
    trace!("kernel:pid[{}] sys_sbrk", current_task().unwrap().pid.0);
    if let Some(old_brk) = current_task().unwrap().change_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}

/// YOUR JOB: Implement spawn.
/// HINT: fork + exec =/= spawn
pub fn sys_spawn(path: *const u8) -> isize {
    trace!(
        "kernel:pid[{}] sys_spawn NOT IMPLEMENTED",
        current_task().unwrap().pid.0
    );
    let token = current_user_token();
    let path = translated_str(token, path);
    if let Some(app_inode) = open_file(path.as_str(), OpenFlags::RDONLY) {
        let add_data = app_inode.read_all();
        let task = current_task().unwrap();
        let new_task = task.spawn(add_data.as_slice());
        let new_pid = new_task.pid.0;
        let trap_cx = new_task.inner_exclusive_access().get_trap_cx();
        trap_cx.x[10] = 0;
        add_task(new_task);
        new_pid as isize
    } else {
        -1
    }
}

// YOUR JOB: Set task priority.
pub fn sys_set_priority(prio: isize) -> isize {
    trace!(
        "kernel:pid[{}] sys_set_priority NOT IMPLEMENTED",
        current_task().unwrap().pid.0
    );
    if prio >= 2 {
        let task = current_task().unwrap();
        task.set_prio(prio);
        prio
    } else {
        -1
    }
}
//https://stackoverflow.com/questions/42499049/transmuting-u8-buffer-to-struct-in-rust
