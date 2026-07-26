---
id: "aetherv-os-roadmap-2026-07-26"
title: "AetherV OS Git Branching Strategy & Roadmap"
type: knowledge-note
source: "User Prompt / Design Specification"
author: "[[Antigravity Coding Assistant]]"
date_created: 2026-07-26
last_modified: 2026-07-26
tags:
  - knowledge/source
  - status/inbox
  - topic/os-development
  - topic/riscv
topics:
  - "[[AetherV OS]]"
  - "[[RISC-V Kernel Development]]"
---

# AetherV OS Git Branching Strategy & Roadmap

## 🎯 Resumen Ejecutivo
> [!abstract] Resumen
> Especificación de la estrategia de ramificación de Git y la hoja de ruta de desarrollo por fases para **AetherV OS** (o **AetherV-RT**), un microkernel minimalista y ligero para la arquitectura RISC-V en QEMU. Define los hitos clave desde el manejo básico de trampas (traps) hasta la evolución a un diseño de microkernel con IPC y aislamiento de espacio de usuario.

---

## 🔑 Conceptos Clave
- **[[AetherV OS]]:** Nombre sugerido para el kernel (Aether: ultra-ligero y limpio; V: RISC-V).
- **[[Trap Handler]]:** Captura y gestión de excepciones y peticiones del sistema (stvec, ecall).
- **[[Sv39 Paging]]:** Mecanismo de paginación de RISC-V para habilitar memoria virtual y aislamiento.
- **[[Preemptive Scheduler]]:** Planificador multitarea basado en interrupciones de temporizador y cambios de contexto.
- **[[FDT (Flattened Device Tree)]]:** Estructura de datos que describe el hardware físico presente para autodescubrimiento.
- **[[Microkernel IPC]]:** Evolución final para desacoplar controladores y servicios a espacio de usuario usando paso de mensajes.

---

## 🗺️ Estrategia de Ramificación (Git Branching Strategy)

Para mantener la estabilidad durante las iteraciones de desarrollo, se adopta el siguiente modelo de ramas estructurado:

```
[ main ]  ---------------------------------------------------> Stable Release (Tested in QEMU / Hardware)


[ develop ]  -----------------------------------------------> Integration & System Tests
├── [ feature/01-trap-handler ]  ------------------------> Interrupter & Trap System (stvec)
├── [ feature/02-sv39-paging ]   ------------------------> Virtual Memory (satp) & User Isolation
├── [ feature/03-preemptive-sched ]  --------------------> Context Switching & Round-Robin Scheduler
├── [ feature/04-devicetree-parser ] --------------------> FDT / Hardware Discovery
├── [ feature/05-virtio-graphics ]  ---------------------> VirtIO GPU & Event Loop (Input)
└── [ feature/06-ipc-microkernel ]  ---------------------> Fast Inter-Process Communication
```

---

## 💡 Ideas Principales / Hoja de Ruta (Core Takeaways)

### Fase 1: Core Foundation (The Bare-Metal Essentials)

#### Rama: `feature/01-trap-handler`
- **Objetivo:** Captura de excepciones de hardware, fallos de página e interrupciones del temporizador.
- **Tareas Clave:**
  1. Escribir el salvador del marco de trampa (trap frame) en ensamblador (`trap.S`) para preservar los 32 registros de propósito general.
  2. Apuntar el registro `stvec` al punto de entrada en ensamblador.
  3. Implementar `rust_trap_handler` para enrutar interrupciones (`timer`, `external`) contra excepciones (`ecall`, `illegal instruction`).
  4. Habilitar la extensión SBI Timer para ticks periódicos.

#### Rama: `feature/02-sv39-paging`
- **Objetivo:** Activar el manejo de memoria virtual bajo el estándar Sv39 de RISC-V.
- **Tareas Clave:**
  1. Definir las estructuras de las tablas de páginas Sv39 (entradas de Nivel 2, 1 y 0).
  2. Implementar mapeo de identidad para el código/datos del Kernel y la RAM física.
  3. Configurar flags de protección de memoria (`R`, `W`, `X`, `U` para espacio de usuario).
  4. Escribir la dirección de la tabla de páginas físicas en el registro `satp` y ejecutar `sfence.vma`.

---

### Fase 2: Multitasking & User Space

#### Rama: `feature/03-preemptive-sched`
- **Objetivo:** Permitir la concurrencia de múltiples hilos/procesos de ejecución.
- **Tareas Clave:**
  1. Definir el Bloque de Control de Procesos (`ProcessControlBlock` - PCB) con estados de registros, puntero de pila y estatus.
  2. Implementar la lógica de cambio de contexto en Ensamblador/Rust (`switch_to`).
  3. Implementar un planificador Round-Robin guiado por las interrupciones del temporizador.
  4. Añadir llamadas al sistema (`sys_yield`, `sys_exit`, `sys_write`) mapeadas mediante `ecall`.

#### Rama: `feature/04-devicetree-parser`
- **Objetivo:** Inspeccionar de forma dinámica la configuración del hardware en el arranque.
- **Tareas Clave:**
  1. Parsear el puntero del Flattened Device Tree (FDT) pasado por OpenSBI en el registro `a1`.
  2. Extraer el tamaño de la memoria, cantidad de hilos de CPU (harts), direcciones de UART y mapeos de regiones MMIO de VirtIO.
  3. Crear una tabla de registro interno de hardware en memoria (`alloc::vec::Vec<Device>`).

---

### Fase 3: Hardware Drivers & I/O Subsystem

#### Rama: `feature/05-virtio-graphics`
- **Objetivo:** Renderizar una interfaz gráfica 2D y manejar entradas de teclado y ratón mediante VirtIO MMIO.
- **Tareas Clave:**
  1. Mapear los registros MMIO de VirtIO (`0x10001000`–`0x10008000`).
  2. Inicializar `virtio-gpu-device` y asignar el Framebuffer en la RAM.
  3. Inicializar los anillos de colas para `virtio-keyboard-device` y `virtio-mouse-device`.
  4. Implementar un bus de eventos asíncrono para entradas del usuario.

---

### Fase 4: Modern Microkernel Evolution

#### Rama: `feature/06-ipc-microkernel`
- **Objetivo:** Desacoplar los controladores y las aplicaciones a procesos de usuario aislados en espacio de usuario (U-Mode / Ring 3).
- **Tareas Clave:**
  1. Implementar comunicación entre procesos (IPC) síncrona y asíncrona de alta velocidad.
  2. Mudar los controladores de VirtIO desde el espacio de kernel hacia procesos en espacio de usuario regulados con tokens de capacidad.
  3. Establecer un registro central de servicios del sistema (File Manager, Network Service, Display Server).

---

## 📌 Flujo de Comandos Git para Fase 1

```bash
# 1. Inicializar repositorio principal
git init
git branch -M main
git commit -m "feat: initial bare-metal RISC-V kernel with OpenSBI and Heap Allocator"

# 2. Crear rama de integración
git checkout -b develop

# 3. Comenzar la primera rama de características
git checkout -b feature/01-trap-handler
```

---

## 🛠️ Tareas / Siguientes Pasos
- [ ] Implementar la primera fase en la rama [[feature/01-trap-handler]]
- [ ] Configurar las tablas de páginas para [[feature/02-sv39-paging]]

---

## 🔗 Notas Relacionadas
- [[README.md]]
- [[src/main.rs]]
- [[src/entry.rs]]

---

