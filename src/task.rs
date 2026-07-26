#![allow(static_mut_refs, dead_code)]
// Planificador preventivo Round-Robin y control de tareas
use crate::sbi;

core::arch::global_asm!(include_str!("switch.S"));

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum TaskStatus {
    Unused,
    Ready,
    Running,
    Blocked,
    Exited,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct TaskContext {
    pub ra: usize,      // Dirección de retorno (donde salta switch_to)
    pub sp: usize,      // Puntero de pila
    pub s: [usize; 12], // Registros callee-saved (s0 - s11)
}

impl TaskContext {
    pub const fn zero() -> Self {
        Self {
            ra: 0,
            sp: 0,
            s: [0; 12],
        }
    }
}

pub struct Task {
    pub id: usize,
    pub kstack: [u8; 4096], // Pila del núcleo de 4KB
    pub ustack: [u8; 4096], // Pila de usuario de 4KB
    pub context: TaskContext,
    pub status: TaskStatus,
}

impl Task {
    pub const fn new(id: usize) -> Self {
        Self {
            id,
            kstack: [0; 4096],
            ustack: [0; 4096],
            context: TaskContext::zero(),
            status: TaskStatus::Unused,
        }
    }
}

const MAX_TASKS: usize = 4;

pub struct SimpleScheduler {
    pub tasks: [Task; MAX_TASKS],
    pub current_id: usize,
}

impl SimpleScheduler {
    pub const fn new() -> Self {
        // Inicializar el planificador estático
        // Tarea 0 representa el hilo principal (rust_main)
        let mut tasks = [
            Task::new(0),
            Task::new(1),
            Task::new(2),
            Task::new(3),
        ];
        tasks[0].status = TaskStatus::Running; // Tarea principal en ejecución
        Self {
            tasks,
            current_id: 0,
        }
    }

    // Cambiar de tarea (Round-Robin)
    pub fn schedule(&mut self) {
        let current_idx = self.current_id;
        let mut next_idx = (current_idx + 1) % MAX_TASKS;

        // Buscar la siguiente tarea lista para ejecutarse
        while next_idx != current_idx {
            if self.tasks[next_idx].status == TaskStatus::Ready {
                break;
            }
            next_idx = (next_idx + 1) % MAX_TASKS;
        }

        // Si no hay ninguna otra tarea lista
        if self.tasks[next_idx].status != TaskStatus::Ready {
            if self.tasks[current_idx].status == TaskStatus::Running {
                return; // Continuar ejecutando la misma tarea
            } else {
                sbi::print_str("\n[Scheduler] No hay tareas listas. Esperando interrupción...\n");
                loop {
                    unsafe {
                        core::arch::asm!("wfi");
                    }
                }
            }
        }

        // Transicionar la tarea actual si estaba en ejecución
        if self.tasks[current_idx].status == TaskStatus::Running {
            self.tasks[current_idx].status = TaskStatus::Ready;
        }

        // Activar la nueva tarea
        self.tasks[next_idx].status = TaskStatus::Running;
        self.current_id = next_idx;

        let old_context_ptr = &mut self.tasks[current_idx].context as *mut TaskContext;
        let new_context_ptr = &self.tasks[next_idx].context as *const TaskContext;

        unsafe {
            extern "C" {
                fn switch_to(old: *mut TaskContext, new: *const TaskContext);
            }
            switch_to(old_context_ptr, new_context_ptr);
        }
    }

    // Salida limpia de la tarea en ejecución
    pub fn exit_current_task(&mut self) -> ! {
        sbi::print_str("\n[Task] Tarea finalizada limpiamente.\n");
        self.tasks[self.current_id].status = TaskStatus::Exited;
        self.schedule();
        loop {
            unsafe {
                core::arch::asm!("wfi");
            }
        }
    }
}

pub static mut SCHEDULER: SimpleScheduler = SimpleScheduler::new();

#[no_mangle]
pub extern "C" fn rust_exit_current_task() -> ! {
    unsafe {
        SCHEDULER.exit_current_task();
    }
}

pub fn create_task(id: usize, entry: fn()) {
    unsafe {
        let scheduler = &mut SCHEDULER;
        if id >= MAX_TASKS {
            panic!("ID de tarea inválido.");
        }
        let task = &mut scheduler.tasks[id];
        task.status = TaskStatus::Ready;

        // Configurar stack frame inicial
        // Al alternar por primera vez, switch_to saltará a task_entry_helper (ensamblador)
        extern "C" {
            fn task_entry_helper();
        }
        task.context.ra = task_entry_helper as *const () as usize;
        task.context.sp = &task.kstack as *const [u8; 4096] as usize + 4096;
        task.context.s[0] = entry as usize; // Guardar entry en s0
    }
}

pub fn yield_cpu() {
    unsafe {
        SCHEDULER.schedule();
    }
}

pub fn create_user_task(id: usize, entry: usize) {
    unsafe {
        let scheduler = &mut SCHEDULER;
        if id >= MAX_TASKS {
            panic!("ID de tarea inválido.");
        }
        let task = &mut scheduler.tasks[id];
        task.status = TaskStatus::Ready;

        // Limpiar el TrapFrame inicial al tope de la pila del kernel de la tarea
        let kstack_top = &task.kstack as *const [u8; 4096] as usize + 4096;
        let tf_ptr = (kstack_top - core::mem::size_of::<crate::trap::TrapFrame>()) as *mut crate::trap::TrapFrame;
        
        // Escribir TrapFrame vacío
        core::ptr::write_volatile(tf_ptr, crate::trap::TrapFrame {
            regs: [0; 32],
            sstatus: 0,
            sepc: entry,
        });

        let tf = &mut *tf_ptr;
        // Configurar pila de usuario (sp = regs[2])
        let ustack_top = &task.ustack as *const [u8; 4096] as usize + 4096;
        tf.regs[2] = ustack_top;

        // Configurar sstatus para volver a U-mode con interrupciones y SUM habilitados
        let mut sstatus: usize;
        core::arch::asm!("csrr {}, sstatus", out(reg) sstatus);
        sstatus &= !(1 << 8); // SPP = 0 (retorno a U-mode)
        sstatus |= 1 << 5;  // SPIE = 1 (habilitar interrupciones en U-mode)
        sstatus |= 1 << 18; // SUM = 1 (acceso de Supervisor a memoria de Usuario)
        tf.sstatus = sstatus;

        // El TaskContext de la tarea guardará:
        // ra: trap_return (para que al alternar ejecute el retorno de trampa)
        // sp: tf_ptr (puntero al TrapFrame que acabamos de crear en su pila del kernel)
        extern "C" {
            fn trap_return();
        }
        task.context.ra = trap_return as *const () as usize;
        task.context.sp = tf_ptr as usize;
    }
}
