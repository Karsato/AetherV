Aquí tienes la documentación del plan en formato **OKF Markdown** (listo para guardar en `docs/aetherv_phase7_5_okf.md`) junto con los comandos de Git para posicionarte en la nueva rama **`feature/07.5-stdio-redirects`** (un nombre alineado con la convención del roadmap) y el prompt para iniciar el desarrollo.

---

### 📄 Archivo para `docs/aetherv_phase7_5_okf.md`

```markdown
---
id: "aetherv-phase7_5-stdio-redirects-2026-07-28"
title: "AetherV OS - Fase 7.5: Redirección de I/O Estándar (STDIO & Log Levels)"
type: knowledge-note
source: "AetherV OS Roadmap"
author: "Antigravity Coding Assistant"
date_created: 2026-07-28
last_modified: 2026-07-28
tags:
  - topic/os-development
  - topic/riscv
  - topic/stdio
  - topic/microkernel
  - topic/logging
topics:
  - "AetherV OS"
  - "Standard I/O Redirection"
  - "LogLevel Filtering"
---

# AetherV OS - Fase 7.5: Redirección de STDIO & Niveles de Log

## 🎯 Resumen Ejecutivo
Diseño e implementación de una capa de **E/S estándar (stdin, stdout, stderr)** y control de **Niveles de Log (LogLevel)** en espacio de usuario (**U-Mode**). Esta mejora permite desacoplar los servidores del sistema (`nameserver`, `gpu_driver_server`, `vfs_server`) de la salida serie directa, filtrando el ruido de inicialización y dejando la consola serial exclusivamente para la **Shell interactiva**, manteniendo las trazas de depuración accesibles según los argumentos de arranque (`loglevel=debug|info|warn|error`).

---

## 🔑 Conceptos Clave & Arquitectura


```

```
                 [ Proceso en U-Mode ]
                           │
           sys_write(fd, buffer, length)
                           │
        ┌──────────────────┴──────────────────┐
        ▼                                     ▼
 [ FD 1: stdout ]                      [ FD 2: stderr ]
        │                                     │

```

Redirigido a Shell / WM              Filtrado por LogLevel
(Salida Limpia de Usuario)            (UART Serial / Debug)

```

1. **Tabla de Descriptores de Archivo (FD Table)**:
   * `FD 0 (stdin)`: Canal de entrada predeterminado (conectado a `input_driver_server`).
   * `FD 1 (stdout)`: Canal de salida estándar (redirigible a consola de la Shell, WM o `/dev/null`).
   * `FD 2 (stderr)`: Canal de diagnóstico y errores (redirigido a la UART serie solo si cumple el criterio de `loglevel`).

2. **Control por `bootargs` (`loglevel`)**:
   * Parsing dinámico en el FDT (`src/fdt.rs`) para configurar la variable global `LOG_LEVEL`:
     * `loglevel=0` (OFF / Quiet): Solo salidas explícitas de la Shell (`stdout`).
     * `loglevel=1` (ERROR): Solo pánicos y fallos críticos.
     * `loglevel=2` (INFO): Mensajes de inicialización de servidores.
     * `loglevel=3` (DEBUG): Trazas completas de cambio de contexto e IPC.

---

## 🛠️ Objetivos & Resultados Clave (OKRs)

- [ ] **KR1.1:** Implementar la abstracción de syscalls `sys_write(fd, buf, len)` y `sys_read(fd, buf, len)` en el Kernel y espacio de usuario.
- [ ] **KR1.2:** Integrar el parser de `loglevel=<level>` en `src/fdt.rs` sobre las propiedades de `bootargs`.
- [ ] **KR1.3:** Migrar las llamadas `user_print(...)` en servidores U-Mode a macros estructuradas (`log_info!`, `log_debug!`, `log_error!`).
- [ ] **KR1.4:** Garantizar un prompt de Shell completamente limpio por defecto sin alterar el código fuente de los servidores.

---

```

---

### 📌 Comandos Git para crear la rama e incluir la documentación

Ejecuta esto en tu terminal para preparar el entorno de trabajo:

```bash
# 1. Crear y cambiar a la rama con el nombre estructurado
git checkout -b feature/07.5-stdio-redirects

# 2. Guardar la documentación OKF de la Fase 7.5
git add docs/aetherv_phase7_5_okf.md
git commit -m "docs(okf): add Phase 7.5 specification for STDIO redirection and LogLevel filtering"

```

---

### 🤖 Prompt para iniciar la Fase 7.5

Copia y pásame este prompt a continuación para arrancar la implementación técnica:

```text
Actúa como experto en arquitectura de microkernels RISC-V y Rust bare-metal (#![no_std]).

Nos encontramos en la rama `feature/07.5-stdio-redirects` tras definir la especificación `docs/aetherv_phase7_5_okf.md`.

Diseña e implementa el sistema de Redirección de STDIO y Filtro de LogLevel para AetherV OS con las siguientes características:

1. Parser de Bootargs en `src/fdt.rs`:
   - Lee la propiedad `bootargs` en el nodo `/chosen` buscando `loglevel=<0..3>` (o `debug`/`quiet`).
   - Setea la variable global `LOG_LEVEL: u8` en `src/task.rs` (0 = OFF, 1 = ERROR, 2 = INFO, 3 = DEBUG).

2. Sistema de Syscalls de E/S Estándar:
   - Reemplaza o extiende `sys_write` para aceptar `fd: usize`:
     - FD 1 (`stdout`): Escribe si el canal está activo o redirigido a la Shell.
     - FD 2 (`stderr`): Escribe en la UART SBI solo si el nivel de mensaje es menor o igual a `LOG_LEVEL`.

3. Macros / Helpers de Logging en U-Mode:
   - Implementa auxiliares `log_error!`, `log_info!`, `log_debug!` que envíen el nivel adecuado a `sys_write`.

4. Refactorización de Servidores:
   - Migra los `user_print` de `nameserver`, `gpu_driver_server`, `input_driver_server` y `vfs_server` a los nuevos helpers sin eliminar el código informativo.

Por favor, genera únicamente los diffs estrictamente necesarios sin explicaciones extensas para optimizar el consumo de tokens.

```
