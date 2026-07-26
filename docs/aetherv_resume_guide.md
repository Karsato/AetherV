# Guía de Continuación de Desarrollo: AetherV OS

Este documento sirve como manual técnico de transferencia para retomar el desarrollo del sistema en la **Fase 4: Modern Microkernel Evolution** (`feature/06-ipc-microkernel`).

---

## 📌 Estado Actual del Proyecto

Hemos implementado con éxito la base de la arquitectura microkernel síncrona en RISC-V con driver de GPU en espacio de usuario (U-Mode):

1.  **Aislamiento U-Mode:** Las tareas corren en U-Mode compartiendo el mapa de memoria `KERNEL_PGTABLE` (Sv39). El código de usuario se mapea de forma transparente con un alias a `0x40000000`.
2.  **Syscalls e IPC síncrono (Rendezvous):** Implementados `sys_ipc_send`, `sys_ipc_recv` y `sys_ipc_reply_recv` con copia directa de datos en memoria kernel y bloqueo selectivo de tareas en el planificador.
3.  **Driver Gráfico en Modo Usuario (`gpu_driver_server`):**
    *   Mapeo de MMIO de VirtIO GPU (`0x10008000`) con permisos de usuario `PTE_U`.
    *   Capa de traducción de direcciones virtuales a físicas (`to_physical`) para que los descriptores DMA del hardware de la GPU resuelvan en la memoria física correcta (`0x80xxxxxx`).
    *   Manejo adaptativo `GPU_ACTIVE` para que el driver no se cuelgue si QEMU no tiene la tarjeta gráfica virtual instanciada.
4.  **Estabilidad del Kernel:**
    *   Preservación del registro temporal `t0` (`x5`) en `src/trap.S` para evitar corrupción de pila del núcleo.
    *   Declaración de `clobber_abi("C")` en todos los bloques `ecall` de U-Mode y S-Mode (`src/sbi.rs`, `src/main.rs`, `src/drivers/gpu.rs`) para evitar que el compilador asuma incorrectamente la preservación de registros caller-saved.
    *   Corrección de Undefined Behavior (UB) en `find_device` en `src/drivers/virtio.rs` reemplazando referencias estáticas por lecturas volátiles explícitas.

---

## 🚀 Cómo Ejecutar y Validar

### Opción A: Simulación Completa (Con GPU Gráfica)
Se requiere la GPU virtual de VirtIO para que el driver en U-mode renderice en pantalla (degradado cromático -> pantalla azul -> degradado cromático):
```bash
qemu-system-riscv64 \
    -machine virt \
    -cpu rv64 \
    -m 256M \
    -bios default \
    -kernel target/riscv64gc-unknown-none-elf/release/microrust-kernel \
    -device virtio-gpu-device \
    -device virtio-keyboard-device \
    -device virtio-mouse-device \
    -nographic
```

### Opción B: Simulación Ligera (Sin GPU Gráfica / Modo Consola)
Para verificar la lógica de IPC y control de errores sin renderizado real:
```bash
qemu-system-riscv64 \
    -machine virt \
    -cpu rv64 \
    -m 128M \
    -nographic \
    -bios default \
    -kernel target/riscv64gc-unknown-none-elf/release/microrust-kernel
```

---

## 🎯 Próximo Paso: Implementar el Registro de Servicios (Name Server)

El **Paso 4** consiste en eliminar los IDs de tareas cableados (hardcoded) en las llamadas IPC de los clientes (ej. `sys_ipc_send(2, &msg)`) mediante un **Nameserver** (Registro de Servicios) en U-mode.

### 1. Arquitectura de Name Server
El Nameserver será una tarea especial del sistema (ej. Tarea 3, `nameserver_server`) que asocie nombres de texto (ej. `"display"`) a puertos o IDs de tareas de servicio reales.

```
┌──────────────────┐               ┌──────────────────┐
│   gpu_client     │               │ gpu_driver_server│
└────────┬─────────┘               └────────▲─────────┘
         │ 1. Lookup("display")             │ 0. Register("display", 2)
         ▼                                  │
┌──────────────────┐                        │
│    nameserver    ├────────────────────────┘
└──────────────────┘
```

### 2. Protocolo de Mensajes del Nameserver
Definir tipos de mensajes específicos en `IpcMessage` para registro y resolución:

```rust
pub const NS_CMD_REGISTER: u32 = 1001;
pub const NS_CMD_LOOKUP:   u32 = 1002;
pub const NS_RESP_SUCCESS: u32 = 2000;
pub const NS_RESP_ERROR:   u32 = 4000;
```

Para la transferencia de cadenas de texto inline, se pueden usar los primeros bytes de `payload`:
*   `payload[0..16]`: Nombre del servicio (ej. `"display\0"`).
*   `payload[16..20]`: Task ID del puerto registrado.

### 3. Plan de Implementación de 4 Pasos

#### Paso A: Crear la Tarea Servidora `nameserver`
1.  En [src/main.rs](file:///home/carlos/PARA/2-frecuente/00/AetherV/src/main.rs), crear una nueva función de tarea `nameserver_task()`.
2.  En ella, mantener un diccionario simple o un vector asociativo para guardar los servicios:
    ```rust
    struct ServiceEntry {
        name: [u8; 16],
        task_id: usize,
    }
    static mut SERVICES: [Option<ServiceEntry>; 8] = [None; 8];
    ```
3.  Procesar las solicitudes en un bucle `sys_ipc_recv` continuo, respondiendo con `sys_ipc_send`.

#### Paso B: Registrar el Nameserver en el Planificador
1.  Asignar al Nameserver un Task ID conocido (ej. Tarea 3) e inicializarlo al arrancar el kernel en `rust_main` mediante `create_user_task`.
2.  Dado que los clientes necesitan saber cómo contactar con el Nameserver al arrancar, el ID del Nameserver (`3`) será la única dirección hardcoded autorizada.

#### Paso C: Registrar el Driver Gráfico en el Nameserver
1.  Al iniciar `gpu_driver_server()`, antes de entrar en su bucle de escucha principal, debe enviar un mensaje de registro al Nameserver:
    *   `dest = 3` (Nameserver)
    *   `msg_type = NS_CMD_REGISTER`
    *   `payload = "display"`
2.  El Nameserver asocia `"display"` con el ID de la tarea emisora (`msg.sender` que será `2`) y devuelve éxito.

#### Paso D: Consultar el Nameserver desde el Cliente
1.  Al iniciar `gpu_client()`, en lugar de hacer `sys_ipc_send(2, &msg)`, primero solicita el ID del servicio de pantalla:
    *   `dest = 3` (Nameserver)
    *   `msg_type = NS_CMD_LOOKUP`
    *   `payload = "display"`
2.  El Nameserver responde devolviendo el ID `2` en el payload.
3.  El cliente extrae el ID de la GPU y realiza su ciclo normal de dibujo enviando las peticiones directamente a dicho ID recuperado de forma dinámica.

---

## 📈 Futuras Fases del Roadmap

Una vez estabilizado el Name Server, el desarrollo de AetherV OS puede seguir la siguiente ruta:
*   **Fase 4.5: Driver de Teclado y Ratón en U-Mode:** Migrar el controlador VirtIO Input a espacio de usuario y notificarle eventos mediante interrupciones físicas delegadas desde el PLIC con `sys_ipc_notify`.
*   **Fase 5: Servidor de Ventanas (Window Manager):** Un proceso U-Mode que actúe de intermediario entre las aplicaciones cliente y el driver GPU, encargándose de mezclar ventanas y buffers en pantalla.
*   **Fase 6: Sistema de Archivos Virtual (VFS Server):** Mover el acceso a disco a un micro-servidor de archivos.
