---
id: "aetherv-phase9-user-shell-2026-07-29"
title: "AetherV OS - Fase 9: Consola de Usuario en Espacio de Usuario (U-Mode Shell)"
type: knowledge-note
source: "AetherV OS Development & Debugging"
author: "Antigravity Coding Assistant"
date_created: 2026-07-29
last_modified: 2026-07-29
tags:
  - topic/os-development
  - topic/riscv
  - topic/user-shell
  - topic/microkernel
  - topic/debugging
topics:
  - "AetherV OS"
  - "U-Mode Shell Task"
  - "Jump Table Page Fault Resolution"
---

# AetherV OS - Fase 9: Consola de Usuario (User Shell) & Resolución de Fallo de Página por Tablas de Salto

## 🎯 Resumen Ejecutivo
Implementación de la **Fase 9: Shell Task** como un proceso interactivo de espacio de usuario (**U-Mode**) en AetherV OS. La Shell actúa como cliente IPC coordinando con el **Nameserver**, **VFS Server**, **Input Server** y el **Window Manager (WM)** para recibir entradas, parsear comandos e imprimir o renderizar en la salida estándar y la interfaz gráfica.

Adicionalmente, se identifica y solventa un fallo crítico de página (`InstructionPageFault`, código `12` / `0x0c`) en el servidor de entrada (`input_driver_server`) causado por optimizaciones del compilador en el mapeo dual de páginas de U-mode.

---

## 🔑 Conceptos Clave & Arquitectura de la Consola

```
   ┌────────────────────────────────────────┐
   │             Shell Task                 │
   │           (U-Mode Task 9)              │
   └────────┬───────────────┬──────────────┬┘
            │               │              │
    Nameserver Lookup   IPC Recv        IPC Send
            │               │              │
            ▼               ▼              ▼
     [ Nameserver ]   [ Input Server ]   [ VFS / WM ]
       (Task 3)          (Task 4)       (Tasks 7 / 2)
```

1. **Resolución e Integración de Servicios**:
   * Al iniciarse, la Shell consulta recursivamente al **Nameserver** (Tarea 3) para enlazar los IDs de tarea de los servidores `vfs`, `input` y `wm`.
   * Permite ejecutar comandos de usuario leídos de forma reactiva a través del bucle de eventos.

2. **Doble Mapeo y Aislamiento de Memoria**:
   * El código del kernel se compila a la dirección base `0x80200000` sin permisos de usuario (`PTE_U`) para la seguridad del modo supervisor (S-mode).
   * Para poder ejecutar las tareas en U-mode, se realiza un mapeo secundario de la sección `.text` en la dirección virtual `0x40200000` con flags `PTE_U` habilitados.

---

## 🐞 Depuración: Resolución de Excepción Fatal (Fallo de Página)

### El Problema
Al arrancar la Shell, el sistema se detenía en una kernel panic:
```
[Exception] Fallo de página detectado (código 0x000000000000000c)
Dirección de fallo (stval): 0x0000000080200976
sepc: 0x0000000080200976
```
* **Dirección de fallo (`sepc`/`stval`)**: `0x80200976` (dentro de la función `keycode_to_char` en `.text` del kernel).
* **Causa raíz**: El compilador había optimizado el largo condicional `if-else` de mapeo de teclas de `keycode_to_char` en una **tabla de saltos** (jump table). Al ser un binario estático enlazado a `0x80200000`, la tabla de saltos contenía direcciones de destino absolutas en la zona `0x8020xxxx`.
* Al ejecutarse la tarea del driver de entrada en U-mode, la CPU intentaba ramificar a estas direcciones del kernel directas (`0x8020xxxx`), que carecen del permiso `PTE_U`, gatillando inmediatamente la violación del control de acceso.

### La Solución
Se reemplazó el condicional/match por una **búsqueda directa mediante arrays estáticos** (`NORMAL_MAP` y `SHIFT_MAP`) en [input_server.rs](file:///home/carlos/PARA/2-frecuente/00/AetherV/src/apps/input_server.rs):
```rust
pub fn keycode_to_char(code: u16, shift: bool) -> Option<char> {
    const NORMAL_MAP: [char; 58] = [ ... ];
    const SHIFT_MAP: [char; 58] = [ ... ];
    let idx = code as usize;
    if idx < 58 {
        let c = if shift { SHIFT_MAP[idx] } else { NORMAL_MAP[idx] };
        if c != '\0' { return Some(c); }
    }
    None
}
```
Esto elimina el match complejo y fuerza al compilador a generar instrucciones de carga indexadas usando direcciones relativas al contador de programa (PC-relative via `auipc`/`addi`), traduciéndose correctamente a la dirección virtual `0x4020xxxx` y eliminando la tabla de saltos absoluta.

---

## 🛠️ Objetivos & Resultados Clave (OKRs) de la Fase 9

- [x] **KR1.1:** Desarrollar `shell_task` en espacio de usuario interactuando mediante IPC.
- [x] **KR1.2:** Enlazar asíncronamente los servicios del Nameserver en el arranque.
- [x] **KR1.3:** Diagnosticar y resolver el bug de Page Fault heredado del compilador por las tablas de salto de `keycode_to_char`.
- [x] **KR1.4:** Comprobar el correcto arranque de la consola interactiva `aetherv-shell>` en QEMU.
