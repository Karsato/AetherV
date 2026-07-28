---
id: "aetherv-phase4_5-input-driver-plan"
title: "AetherV OS - Planificación Fase 4.5: Driver de Entrada en U-Mode"
type: knowledge-note
source: "AetherV OS Roadmap"
author: "Antigravity Coding Assistant"
date_created: 2026-07-27
last_modified: 2026-07-27
tags:
  - topic/os-development
  - topic/riscv
  - topic/drivers
  - topic/virtio
  - topic/ipc
---

# Plan de Implementación: Fase 4.5 (Driver de Teclado y Ratón en U-Mode) - ¡COMPLETADO!

Este documento detalla el plan técnico para implementar la **Fase 4.5** de AetherV OS, la cual ha sido completada exitosamente.

---

## 🎯 Objetivo
Migrar el controlador físico de **VirtIO Input** (teclado y ratón) de QEMU a espacio de usuario (**U-Mode**). La interacción se basa en la delegación de interrupciones físicas de hardware desde el kernel mediante notificaciones asíncronas de IPC (`sys_ipc_notify`).

---

## 🔑 Conceptos de Arquitectura

```
  [ Hardware (QEMU) ] ──(Interrupción Física)──> [ PLIC / Kernel ]
                                                        │
                                                 (sys_ipc_notify)
                                                        │
                                                        ▼
  [ U-Mode Client ] <──(IPC Síncrono)── [ input_driver_server ]
```

1. **Aislamiento MMIO (PTE_U):** El kernel mapea el registro físico del dispositivo VirtIO Input (Device ID 18) en el espacio de usuario de forma que el servidor de entrada pueda manipular directamente las colas del dispositivo sin entrar a modo privilegiado.
2. **Mecanismo de Interrupciones en Microkernel:** 
   - El teclado de QEMU genera una interrupción de hardware externa que llega al PLIC y es atrapada por el kernel.
   - El kernel realiza el "Acknowledge" y, en lugar de leer el dispositivo, envía un `sys_ipc_notify` con una máscara de bits al proceso del driver en U-Mode.
   - El driver recibe la notificación de forma asíncrona, consume los descriptores de la Virtqueue del dispositivo de entrada y extrae el scancode del teclado.

---

## 🛠️ Plan de Implementación y Estado de Pasos

### Paso A: Mapeo MMIO de Entrada - **Completado**
- Se localiza y mapea el dispositivo `VirtIO Keyboard` dinámicamente verificando su nombre a nivel de MMIO (para distinguirlo del mouse).
- Se mapea su dirección física en la tabla de páginas del kernel en [paging.rs](file:///home/carlos/PARA/2-frecuente/00/AetherV/src/paging.rs) con los flags `PTE_R | PTE_W | PTE_U` para dar acceso de usuario.

### Paso B: Creación del Servidor `input_driver_server` - **Completado**
- Definida la tarea `input_driver_server` en [main.rs](file:///home/carlos/PARA/2-frecuente/00/AetherV/src/main.rs) y registrada en el planificador (Tarea ID 4).
- Inicializadas las colas (Virtqueues) en [input.rs](file:///home/carlos/PARA/2-frecuente/00/AetherV/src/drivers/input.rs) traduciendo direcciones con la capa `to_physical` en modo Legacy PFN.
- Registrado el servicio `"input"` en el [nameserver](file:///home/carlos/PARA/2-frecuente/00/AetherV/src/main.rs#L148) (Tarea ID 3).

### Paso C: Delegación de Interrupciones PLIC - **Completado**
- Habilitadas las interrupciones externas para teclado (**IRQ 7**) y ratón (**IRQ 6**) en el PLIC y configurado el bit `SEIE` en el manejador en [trap.rs](file:///home/carlos/PARA/2-frecuente/00/AetherV/src/trap.rs).
- En el manejador de trampas, al capturar el IRQ del teclado, llamamos a `sys_ipc_notify(4, 1)` para despertar al servidor de entrada.

### Paso D: Procesamiento de Eventos de Entrada - **Completado**
- En el bucle principal de `input_driver_server`, se reciben las notificaciones asíncronas y se consume el Used Ring de la Virtqueue para extraer el scancode decodificado a caracteres ASCII en una cola circular.
- Se implementó un cliente `input_client` (Tarea ID 5) que consulta el driver de forma periódica con `INPUT_CMD_GET_KEY` y recibe caracteres válidos.

---

## ⚡ Optimización de Bajo Consumo (Wait For Interrupt - WFI) - **Completado**

Para evitar el uso innecesario del 100% de la CPU en la máquina anfitriona (host):
1. **Tarea Idle (Task 0 / Main Thread):** Tras completar las iteraciones iniciales de verificación, la tarea principal del núcleo en S-Mode entra en un bucle que ejecuta la instrucción assembly `wfi` y luego cede la CPU de forma cooperativa.
2. **Planificador Eficiente (`src/task.rs`):** El planificador Round-Robin se optimizó para que en caso de no haber ninguna tarea lista (`TaskStatus::Ready`), suspenda la CPU ejecutando la instrucción `wfi` en un bucle sin hacer llamadas recursivas de pila, despertando únicamente al llegar interrupciones externas (temporizador o PLIC).
