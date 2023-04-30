//! Task management implementation
//!
//! Everything about task management, like starting and switching tasks is
//! implemented here.
//!
//! A single global instance of [`TaskManager`] called `TASK_MANAGER` controls
//! all the tasks in the whole operating system.
//!
//! A single global instance of [`Processor`] called `PROCESSOR` monitors running
//! task(s) for each core.
//!
//! A single global instance of `PID_ALLOCATOR` allocates pid for user apps.
//!
//! Be careful when you see `__switch` ASM function in `switch.S`. Control flow around this function
//! might not be what you expect.
mod context;
mod id;
mod manager;
mod processor;
mod switch;
#[allow(clippy::module_inception)]
mod task;

use crate::loader::get_app_data_by_name;
use alloc::sync::Arc;

// use crate::config::MAX_SYSCALL_NUM;
// use crate::loader::{get_app_data, get_num_app};
// use crate::mm::{VPNRange, MapPermission};
// use crate::sync::UPSafeCell;
// use crate::trap::TrapContext;
// use alloc::vec::Vec;
use lazy_static::*;
pub use manager::{fetch_task, TaskManager};
use switch::__switch;
pub use task::{TaskControlBlock, TaskStatus};

// use crate::config::MAX_SYSCALL_NUM;
// use crate::loader::{get_app_data, get_num_app};
// use crate::mm::{VirtAddr, MapPermission};
// use crate::sync::UPSafeCell;
// use crate::trap::TrapContext;
// use alloc::vec::Vec;

pub use context::TaskContext;
pub use id::{kstack_alloc, pid_alloc, KernelStack, PidHandle};
pub use manager::add_task;
pub use processor::{
    current_task, current_trap_cx, current_user_token, run_tasks, schedule,
    take_current_task, Processor, current_sys_time, current_status, current_syscall_times
};

/// Suspend the current 'Running' task and run the next task in task list.
pub fn suspend_current_and_run_next() {
    // There must be an application running.
    let task = take_current_task().unwrap();

    // ---- access current TCB exclusively
    let mut task_inner = task.inner_exclusive_access();
    let task_cx_ptr = &mut task_inner.task_cx as *mut TaskContext;
    // Change status to Ready
    task_inner.task_status = TaskStatus::Ready;
    drop(task_inner);
    // ---- release current PCB

    // push back to ready queue.
    add_task(task);
    // jump to scheduling cycle
    schedule(task_cx_ptr);
}

/// pid of usertests app in make run TEST=1
pub const IDLE_PID: usize = 0;

/// Exit the current 'Running' task and run the next task in task list.
pub fn exit_current_and_run_next(exit_code: i32) {
    // take from Processor let task = take_current_task().unwrap();

    let task = current_task().unwrap();

    let pid = task.getpid();
    if pid == IDLE_PID {
        println!(
            "[kernel] Idle process exit with exit_code {} ...",
            exit_code
        );
        panic!("All applications completed!");
    }

    // **** access current TCB exclusively
    let mut inner = task.inner_exclusive_access();
    // Change status to Zombie
    inner.task_status = TaskStatus::Zombie;
    // Record exit code
    inner.exit_code = exit_code;
    // do not move to its parent but under initproc

    // ++++++ access initproc TCB exclusively
    {
        let mut initproc_inner = INITPROC.inner_exclusive_access();
        for child in inner.children.iter() {
            child.inner_exclusive_access().parent = Some(Arc::downgrade(&INITPROC));
            initproc_inner.children.push(child.clone());
        }
    }
    // ++++++ release parent PCB

    inner.children.clear();
    // deallocate user space
    inner.memory_set.recycle_data_pages();
    drop(inner);
    // **** release current PCB
    // drop task manually to maintain rc correctly
    drop(task);
    // we do not have to save task context
    let mut _unused = TaskContext::zero_init();
    schedule(&mut _unused as *mut _);
}

lazy_static! {
    /// Creation of initial process
    ///
    /// the name "initproc" may be changed to any other app name like "usertests",
    /// but we have user_shell, so we don't need to change it.
    pub static ref INITPROC: Arc<TaskControlBlock> = Arc::new(TaskControlBlock::new(
        get_app_data_by_name("ch5b_initproc").unwrap()
    ));
}

///Add init process to the manager
pub fn add_initproc() {
    add_task(INITPROC.clone());
}
// Generally, the first task in task list is an idle task (we call it zero process later).
// But in ch4, we load apps statically, so the first task is a real app.
//     fn run_first_task(&self) -> ! {
//         let mut inner = self.inner.exclusive_access();
//         let next_task = &mut inner.tasks[0];
//         next_task.task_status = TaskStatus::Running;
//         let next_task_cx_ptr = &next_task.task_cx as *const TaskContext;
//         drop(inner);
//         let mut _unused = TaskContext::zero_init();
//         // before this, we should drop local variables that must be dropped manually
//         unsafe {
//             __switch(&mut _unused as *mut _, next_task_cx_ptr);
//         }
//         panic!("unreachable in run_first_task!");
//     }
//
//     // Change the status of current `Running` task into `Ready`.
//     fn mark_current_suspended(&self) {
//         let mut inner = self.inner.exclusive_access();
//         let cur = inner.current_task;
//         inner.tasks[cur].task_status = TaskStatus::Ready;
//     }
//
//     // Change the status of current `Running` task into `Exited`.
//     fn mark_current_exited(&self) {
//         let mut inner = self.inner.exclusive_access();
//         let cur = inner.current_task;
//         inner.tasks[cur].task_status = TaskStatus::Exited;
//     }
//
//     // Find next task to run and return task id.
//     //
//     // In this case, we only return the first `Ready` task in task list.
//     fn find_next_task(&self) -> Option<usize> {
//         let inner = self.inner.exclusive_access();
//         let current = inner.current_task;
//         (current + 1..current + self.num_app + 1)
//             .map(|id| id % self.num_app)
//             .find(|id| inner.tasks[*id].task_status == TaskStatus::Ready)
//     }
//
//     // Get the current 'Running' task's token.
//     fn get_current_token(&self) -> usize {
//         let inner = self.inner.exclusive_access();
//         inner.tasks[inner.current_task].get_user_token()
//     }
//
//     // Get the current 'Running' task's status
//     fn get_current_status(&self) -> TaskStatus {
//         let inner = self.inner.exclusive_access();
//         inner.tasks[inner.current_task].get_task_status()
//     }
//     // Get the current 'Running' task's syscall times
//     fn get_syscall_times(&self) -> [u32; MAX_SYSCALL_NUM] {
//         let inner = self.inner.exclusive_access();
//         inner.tasks[inner.current_task].syscall_times
//     }
//
//     // get memset
//     fn insert_map(&self, start_va: VirtAddr, en#d_va: VirtAddr, permission: MapPermission) {
//         let mut inner = self.inner.exclusive_access();
//         let current_task = inner.current_task;
//         inner.tasks[current_task].memory_set.insert_framed_area(start_va, end_va, permission)
//     }
//
//     // remove memset
//     fn remove_map(&self, start_va: VirtAddr, end_va: VirtAddr) {
//         let mut inner = self.inner.exclusive_access();
//         let current_task = inner.current_task;
//         inner.tasks[current_task].memory_set.remove_map_area(start_va, end_va);
//     }
//
//     // Get the current 'Running' task's sys time
//     fn get_sys_time(&self) -> usize {
//         let inner = self.inner.exclusive_access();
//         inner.tasks[inner.current_task].sys_time
//     }
//     // record syscall
//     fn record_syscall(&self, syscall_id: usize) {
//         let mut inner = self.inner.exclusive_access();
//         let current_task = inner.current_task;
//         inner.tasks[current_task].syscall_times[syscall_id] += 1;
//     }
//
//     // Get the current 'Running' task's trap contexts.
//     fn get_current_trap_cx(&self) -> &'static mut TrapContext {
//         let inner = self.inner.exclusive_access();
//         inner.tasks[inner.current_task].get_trap_cx()
//     }
//
//     // Change the current 'Running' task's program break
//     pub fn change_current_program_brk(&self, size: i32) -> Option<usize> {
//         let mut inner = self.inner.exclusive_access();
//         let cur = inner.current_task;
//         inner.tasks[cur].change_program_brk(size)
//     }
//
//     // Switch current `Running` task to the task we have found,
//     // or there is no `Ready` task and we can exit with all applications completed
//     fn run_next_task(&self) {
//         if let Some(next) = self.find_next_task() {
//             let mut inner = self.inner.exclusive_access();
//             let current = inner.current_task;
//             inner.tasks[next].task_status = TaskStatus::Running;
//             inner.current_task = next;
//             let current_task_cx_ptr = &mut inner.tasks[current].task_cx as *mut TaskContext;
//             let next_task_cx_ptr = &inner.tasks[next].task_cx as *const TaskContext;
//             drop(inner);
//             // before this, we should drop local variables that must be dropped manually
//             unsafe {
//                 __switch(current_task_cx_ptr, next_task_cx_ptr);
//             }
//             // go back to user mode
//         } else {
//             panic!("All applications completed!");
//         }
//     }
// }
//
// // Run the first task in task list.
// pub fn run_first_task() {
//     TASK_MANAGER.run_first_task();
// }
//
// // Switch current `Running` task to the task we have found,
// // or there is no `Ready` task and we can exit with all applications completed
// fn run_next_task() {
//     TASK_MANAGER.run_next_task();
// }
//
// // Change the status of current `Running` task into `Ready`.
// fn mark_current_suspended() {
//     TASK_MANAGER.mark_current_suspended();
// }
//
// // Change the status of current `Running` task into `Exited`.
// fn mark_current_exited() {
//     TASK_MANAGER.mark_current_exited();
// }
//
// // Suspend the current 'Running' task and run the next task in task list.
// pub fn suspend_current_and_run_next() {
//     mark_current_suspended();
//     run_next_task();
// }
//
// // Exit the current 'Running' task and run the next task in task list.
// pub fn exit_current_and_run_next() {
//     mark_current_exited();
//     run_next_task();
// }
//
// // Get the current 'Running' task's token.
// pub fn current_user_token() -> usize {
//     TASK_MANAGER.get_current_token()
// }
//
// // Get the current 'Running' task's trap contexts.
// pub fn current_trap_cx() -> &'static mut TrapContext {
//     TASK_MANAGER.get_current_trap_cx()
// }
//
// Get the current 'Running' task's status
// pub fn current_status() -> TaskStatus{
//     TASK_MANAGER.get_current_status()
// }

// // Get the current 'Running' task's times
// pub fn current_syscall_times() -> [u32; MAX_SYSCALL_NUM]{
//     TASK_MANAGER.get_syscall_times()
// }
//
// // Get the current 'Running' task's time
// pub fn current_sys_time() -> usize {
//     TASK_MANAGER.get_sys_time()
// }
//
// // record syscall
// pub fn record_syscall(syscall_id: usize) {
//     TASK_MANAGER.record_syscall(syscall_id)
// }
//
// // Change the current 'Running' task's program break
// pub fn change_program_brk(size: i32) -> Option<usize> {
//     TASK_MANAGER.change_current_program_brk(size)
// >>>>>>> d0c7992 (add but not finish)
// }
//
// //GET current memset
// pub fn insert_map(start_va: VirtAddr, end_va: VirtAddr, permission: MapPermission) {
//     TASK_MANAGER.insert_map(start_va, end_va, permission);
// }
//
// // remove current memset
// pub fn remove_map(start_va: VirtAddr, end_va: VirtAddr) {
//     TASK_MANAGER.remove_map(start_va, end_va);
// }
