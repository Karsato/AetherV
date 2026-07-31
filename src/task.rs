#![allow(static_mut_refs, dead_code)]
// Planificador preventivo Round-Robin y control de tareas
use crate::sbi;

core::arch::global_asm!(include_str!("switch.S"));

pub static mut LOG_LEVEL: u8 = 0; // 0=OFF/Quiet, 1=ERROR, 2=INFO, 3=DEBUG
pub static mut DEBUG_LOGS: bool = false;

#[inline(always)]
pub fn get_log_level() -> u8 {
    unsafe { LOG_LEVEL }
}

#[inline(always)]
pub fn is_debug_enabled() -> bool {
    unsafe { DEBUG_LOGS }
}

pub fn log_debug(f: impl FnOnce()) {
    if unsafe { DEBUG_LOGS } {
        f();
    }
}
pub const IPC_WILDCARD: usize = usize::MAX;
pub const IPC_SENDER_NOTIFICATION: u32 = 0xFFFFFFFF;
pub const IPC_MSG_NOTIFICATION: u32 = 0xFFFFFFFF;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct IpcMessage {
    pub sender: u32,
    pub msg_type: u32,
    pub length: u32,
    pub reserved: u32,
    pub payload: [u8; 32],
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum TaskStatus {
    Unused,
    Ready,
    Running,
    BlockedSend, // Bloqueado esperando que el receptor acepte el mensaje
    BlockedRecv, // Bloqueado esperando recibir un mensaje
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
    pub tf_addr: usize,         // Dirección física/virtual del TrapFrame en pila de núcleo
    pub ipc_partner: usize,     // ID de la tarea con la que intenta comunicarse
    pub ipc_buffer_ptr: usize,  // Puntero virtual al mensaje IpcMessage
    pub ipc_notifications: u32,  // Notificaciones asíncronas acumuladas
    pub last_sender: usize,      // Último emisor atendido por esta tarea (Round-Robin IPC)
}

impl Task {
    pub const fn new(id: usize) -> Self {
        Self {
            id,
            kstack: [0; 4096],
            ustack: [0; 4096],
            context: TaskContext::zero(),
            status: TaskStatus::Unused,
            tf_addr: 0,
            ipc_partner: 0,
            ipc_buffer_ptr: 0,
            ipc_notifications: 0,
            last_sender: 0,
        }
    }
}

pub const MAX_TASKS: usize = 16;

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
            Task::new(4),
            Task::new(5),
            Task::new(6),
            Task::new(7),
            Task::new(8),
            Task::new(9),
            Task::new(10),
            Task::new(11),
            Task::new(12),
            Task::new(13),
            Task::new(14),
            Task::new(15),
        ];
        tasks[0].status = TaskStatus::Running; // Tarea principal en ejecución
        Self {
            tasks,
            current_id: 0,
        }
    }

    // Cambiar de tarea (Round-Robin)
    pub fn schedule(&mut self) {
        loop {
            let current_idx = self.current_id;
            let mut next_idx = (current_idx + 1) % MAX_TASKS;

            // Buscar la siguiente tarea lista para ejecutarse
            while next_idx != current_idx {
                if self.tasks[next_idx].status == TaskStatus::Ready {
                    break;
                }
                next_idx = (next_idx + 1) % MAX_TASKS;
            }

            // Si hay una tarea lista para ejecutarse
            if self.tasks[next_idx].status == TaskStatus::Ready {
                // Transicionar la tarea actual si estaba en ejecución
                if self.tasks[current_idx].status == TaskStatus::Running {
                    self.tasks[current_idx].status = TaskStatus::Ready;
                }

                // Imprimir traza de cambio de contexto
                log_debug(|| {
                    sbi::print_str("[Scheduler] Cambiando de Tarea ");
                    sbi::print_hex(current_idx);
                    sbi::print_str(" a Tarea ");
                    sbi::print_hex(next_idx);
                    sbi::print_str("\n");
                });

                // Activar la nueva tarea
                self.tasks[next_idx].status = TaskStatus::Running;
                self.current_id = next_idx;

                let old_context_ptr = &mut self.tasks[current_idx].context as *mut TaskContext;
                let new_context_ptr = &self.tasks[next_idx].context as *const TaskContext;

                unsafe {
                    extern "C" {
                        fn switch_to(old: *mut TaskContext, new: *const TaskContext);
                    }
                    sbi::sbi_set_timer(sbi::get_time() + crate::trap::TIMER_INTERVAL);
                    switch_to(old_context_ptr, new_context_ptr);
                }
                return;
            }

            // Si no hay ninguna otra tarea lista pero la actual sigue en ejecución
            if self.tasks[current_idx].status == TaskStatus::Running {
                return; // Continuar ejecutando la misma tarea
            }

            // Si no hay ninguna tarea lista en absoluto, suspender la CPU hasta la próxima interrupción
            sbi::print_str("\n[Scheduler] No hay tareas listas. Suspendiendo CPU (WFI)...\n");
            unsafe {
                core::arch::asm!("csrs sstatus, {}", in(reg) (1 << 1));
                core::arch::asm!("wfi");
            }
            sbi::sbi_set_timer(sbi::get_time() + crate::trap::TIMER_INTERVAL);
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

pub fn sys_ipc_send(dest_id: usize, msg_ptr: usize) -> isize {
    unsafe {
        let sender_id = SCHEDULER.current_id;
        log_debug(|| {
            sbi::print_str("[IPC DEBUG] sys_ipc_send from ");
            sbi::print_hex(sender_id);
            sbi::print_str(" to ");
            sbi::print_hex(dest_id);
            sbi::print_str("\n");
        });
        if dest_id >= MAX_TASKS || dest_id == sender_id {
            return -1; // Destino inválido
        }
        let dest = &mut SCHEDULER.tasks[dest_id];
        if dest.status == TaskStatus::Unused || dest.status == TaskStatus::Exited {
            return -2; // Destino no está activo
        }

        // Comprobar si el receptor ya está esperando un mensaje de nosotros (o de cualquiera)
        if dest.status == TaskStatus::BlockedRecv && (dest.ipc_partner == sender_id || dest.ipc_partner == IPC_WILDCARD) {
            // Rendezvous!
            log_debug(|| {
                sbi::print_str("[IPC DEBUG] sys_ipc_send rendezvous from ");
                sbi::print_hex(sender_id);
                sbi::print_str(" to ");
                sbi::print_hex(dest_id);
                sbi::print_str("\n");
            });
            let dest_buf_ptr = dest.ipc_buffer_ptr;
            
            // Copiar mensaje
            core::ptr::copy_nonoverlapping(
                msg_ptr as *const u8,
                dest_buf_ptr as *mut u8,
                core::mem::size_of::<IpcMessage>(),
            );

            // Escribir remitente en el mensaje destino
            let dest_msg = &mut *(dest_buf_ptr as *mut IpcMessage);
            dest_msg.sender = sender_id as u32;

            // Desbloquear al receptor
            dest.status = TaskStatus::Ready;
            
            // Escribir 0 (éxito) en a0 de la tarea receptora
            let dest_tf = dest.tf_addr as *mut crate::trap::TrapFrame;
            if dest_tf as usize != 0 {
                (*dest_tf).regs[10] = 0; // regs[10] es a0
            }

            return 0; // Éxito
        } else {
            // No hay nadie esperando, bloquear al emisor
            log_debug(|| {
                sbi::print_str("[IPC] Bloqueando emisor ");
                sbi::print_hex(sender_id);
                sbi::print_str(" esperando a ");
                sbi::print_hex(dest_id);
                sbi::print_str("\n");
            });

            let sender = &mut SCHEDULER.tasks[sender_id];
            sender.status = TaskStatus::BlockedSend;
            sender.ipc_partner = dest_id;
            sender.ipc_buffer_ptr = msg_ptr;

            // Cambiar de contexto
            SCHEDULER.schedule();
            return 0;
        }
    }
}

pub fn sys_ipc_recv(src_id: usize, msg_ptr: usize) -> isize {
    unsafe {
        let receiver_id = SCHEDULER.current_id;
        log_debug(|| {
            sbi::print_str("[IPC DEBUG] sys_ipc_recv receiver ");
            sbi::print_hex(receiver_id);
            sbi::print_str(" from ");
            sbi::print_hex(src_id);
            sbi::print_str("\n");
        });
        if src_id != IPC_WILDCARD && src_id >= MAX_TASKS {
            return -1; // Origen inválido
        }

        // Comprobar si hay notificaciones pendientes si se acepta cualquiera
        let receiver = &mut SCHEDULER.tasks[receiver_id];
        if src_id == IPC_WILDCARD && receiver.ipc_notifications != 0 {
            // Entregar notificación como mensaje especial
            let msg = &mut *(msg_ptr as *mut IpcMessage);
            msg.sender = IPC_SENDER_NOTIFICATION;
            msg.msg_type = IPC_MSG_NOTIFICATION;
            msg.length = 4;
            // Escribir el bitmask en el payload
            let bits_bytes = receiver.ipc_notifications.to_ne_bytes();
            msg.payload[0..4].copy_from_slice(&bits_bytes);
            
            receiver.ipc_notifications = 0; // Limpiar notificaciones
            return 0; // Retorno inmediato con éxito
        }

        // Buscar si hay algún emisor bloqueado queriendo enviarnos un mensaje
        // Usar búsqueda Round-Robin propia de la tarea para evitar la inanición (starvation) de tareas con IDs altos
        let mut found_sender_id = None;
        let start = (receiver.last_sender + 1) % MAX_TASKS;
        for offset in 0..MAX_TASKS {
            let i = (start + offset) % MAX_TASKS;
            let t = &SCHEDULER.tasks[i];
            if t.status == TaskStatus::BlockedSend && t.ipc_partner == receiver_id {
                if src_id == IPC_WILDCARD || src_id == i {
                    found_sender_id = Some(i);
                    receiver.last_sender = i;
                    break;
                }
            }
        }

        if let Some(sender_id) = found_sender_id {
            // Rendezvous!
            log_debug(|| {
                sbi::print_str("[IPC DEBUG] sys_ipc_recv rendezvous receiver ");
                sbi::print_hex(receiver_id);
                sbi::print_str(" from ");
                sbi::print_hex(sender_id);
                sbi::print_str("\n");
            });
            let sender = &mut SCHEDULER.tasks[sender_id];
            let sender_buf_ptr = sender.ipc_buffer_ptr;

            // Copiar mensaje
            core::ptr::copy_nonoverlapping(
                sender_buf_ptr as *const u8,
                msg_ptr as *mut u8,
                core::mem::size_of::<IpcMessage>(),
            );

            // Escribir remitente
            let dest_msg = &mut *(msg_ptr as *mut IpcMessage);
            dest_msg.sender = sender_id as u32;

            // Desbloquear emisor
            sender.status = TaskStatus::Ready;
            let sender_tf = sender.tf_addr as *mut crate::trap::TrapFrame;
            if sender_tf as usize != 0 {
                (*sender_tf).regs[10] = 0; // Retornar 0 (éxito) en a0 de la tarea emisora
            }

            return 0; // Éxito
        } else {
            // Bloquear al receptor
            log_debug(|| {
                sbi::print_str("[IPC] Bloqueando receptor ");
                sbi::print_hex(receiver_id);
                sbi::print_str(" esperando a ");
                sbi::print_hex(src_id);
                sbi::print_str("\n");
            });

            let receiver = &mut SCHEDULER.tasks[receiver_id];
            receiver.status = TaskStatus::BlockedRecv;
            receiver.ipc_partner = src_id;
            receiver.ipc_buffer_ptr = msg_ptr;

            // Cambiar de contexto
            SCHEDULER.schedule();
            return 0;
        }
    }
}

pub fn sys_ipc_notify(dest_id: usize, bits: u32) -> isize {
    unsafe {
        if dest_id >= MAX_TASKS {
            return -1;
        }
        let dest = &mut SCHEDULER.tasks[dest_id];
        if dest.status == TaskStatus::Unused || dest.status == TaskStatus::Exited {
            return -2;
        }

        // Acumular bits de notificación
        dest.ipc_notifications |= bits;

        // Si el destino está bloqueado esperando un mensaje de cualquiera (wildcard), despertarlo con la notificación
        if dest.status == TaskStatus::BlockedRecv && dest.ipc_partner == IPC_WILDCARD {
            let msg_ptr = dest.ipc_buffer_ptr;
            let msg = &mut *(msg_ptr as *mut IpcMessage);
            msg.sender = IPC_SENDER_NOTIFICATION;
            msg.msg_type = IPC_MSG_NOTIFICATION;
            msg.length = 4;
            let bits_bytes = dest.ipc_notifications.to_ne_bytes();
            msg.payload[0..4].copy_from_slice(&bits_bytes);

            dest.ipc_notifications = 0; // Limpiar
            dest.status = TaskStatus::Ready;

            let dest_tf = dest.tf_addr as *mut crate::trap::TrapFrame;
            if dest_tf as usize != 0 {
                (*dest_tf).regs[10] = 0; // Retornar 0 (éxito) en a0
            }
        }

        0 // Retorno de éxito (no bloqueante)
    }
}

pub fn sys_ipc_reply_recv(dest_id: usize, reply_msg_ptr: usize, src_id: usize, recv_msg_ptr: usize) -> isize {
    // 1. Enviar respuesta (send síncrono al receptor bloqueado)
    let send_res = sys_ipc_send(dest_id, reply_msg_ptr);
    if send_res != 0 {
        return send_res;
    }
    // 2. Bloquearse esperando el siguiente mensaje (recv síncrono)
    sys_ipc_recv(src_id, recv_msg_ptr)
}
