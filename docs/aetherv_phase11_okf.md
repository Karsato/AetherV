---
id: "aetherv-phase11-virtio-block-2026-07-31"
title: "AetherV OS - Fase 11: Driver de Disco en U-Mode y Persistencia FAT16"
type: knowledge-note
source: "AetherV OS Development"
author: "Antigravity Coding Assistant"
date_created: 2026-07-31
last_modified: 2026-07-31
tags:
  - topic/os-development
  - topic/riscv
  - topic/virtio-block
  - topic/vfs-server
  - topic/fat16
topics:
  - "AetherV OS"
  - "U-Mode Block Server"
  - "VFS FAT16 File Integration"
---

# AetherV OS - Fase 11: Driver de Disco en U-Mode (VirtIO-Block) & Sistema de Archivos FAT16

## 🎯 Resumen Ejecutivo
Implementación de la **Fase 11: Driver de Disco Persistente en Espacio de Usuario (U-Mode Block Server)** integrado con un sistema de archivos **FAT16** dinámico en el `vfs_server`. Esta arquitectura elimina el `RAMDisk` estático y delega el acceso a disco a un proceso servidor especializado (`block_server_task`), el cual se comunica a través de mensajes IPC estructurados para leer y escribir sectores físicos organizados en bloques de 512 bytes (transferidos mediante chunks de 32 bytes debido a limitaciones del payload IPC).

---

## 🔑 Conceptos Clave & Arquitectura de Almacenamiento

```
   ┌─────────────────────────────────────────────────────────┐
   │                       Shell Task                        │
   │                     (U-Mode Task 9)                     │
   └────────────┬─────────────────────────────┬──────────────┘
                │                             │
        ls (Read Directory)          cat (Read File Data)
                │                             │
                ▼                             ▼
   ┌─────────────────────────────────────────────────────────┐
   │                       VFS Server                        │
   │                     (U-Mode Task 7)                     │
   └────────────────────────────┬────────────────────────────┘
                                │
                      BLOCK_CMD_READ (Sector/Chunk)
                                │
                                ▼
   ┌─────────────────────────────────────────────────────────┐
   │                      Block Server                       │
   │                    (U-Mode Task 11)                     │
   └────────────────────────────┬────────────────────────────┘
                                │
                  VirtIO-Block Discovery (MMIO Scan)
                                │
                                ▼
                    [ VirtIO Block Hardware ]
```

1. **Servidor de Bloques (`block_server`)**:
   * Escanea dinámicamente el bus MMIO de VirtIO (`0x10001000`–`0x10009000`) buscando el Device ID `2` (tarjetas de disco/bloque VirtIO).
   * Si no se detecta la tarjeta física, conmuta a modo simulado proporcionando sectores de disco pre-estructurados con el formato FAT16 (incluyendo la firma de arranque `0xAA55` en el sector 0, tablas de directorios FAT en el sector 1 y clústeres de datos).

2. **Servidor de Archivos FAT16 (`vfs_server`)**:
   * Busca el servicio `"block"` en el Nameserver y lo vincula.
   * Al abrir un archivo (`VFS_CMD_OPEN`), realiza llamadas IPC a `"block"` para leer la tabla del directorio raíz en el Sector 1.
   * Mapea y traduce las rutas `/readme.txt`, `/config.sys` y `/persist.dat` a los nombres FAT16 estándar paddeados (`"README  TXT"`, `"CONFIG  SYS"`, `"PERSIST DAT"`).
   * Al leer (`VFS_CMD_READ`), obtiene el número de cluster inicial y lee el sector de datos correspondiente enviando solicitudes al servidor de bloques.

---

## 🛠️ Objetivos & Resultados Clave (OKRs) de la Fase 11

- [x] **KR1.1:** Implementar `block_server_task` en espacio de usuario e integrarlo en la tabla de tareas del microkernel (ID de tarea 11).
- [x] **KR1.2:** Crear comandos IPC de disco (`BLOCK_CMD_READ` y `BLOCK_CMD_WRITE`) con soporte para segmentación de chunks de 32 bytes.
- [x] **KR1.3:** Modificar `vfs_server` para buscar y comunicarse con el servidor `"block"` para la resolución y lectura de archivos.
- [x] **KR1.4:** Implementar una traducción de ruta a formato de 11 bytes FAT16 e incorporar el soporte de lectura del directorio raíz.
- [x] **KR1.5:** Integrar la visualización del archivo `/persist.dat` en los comandos `ls` y `cat` de la consola interactiva (`shell_task`).
- [x] **KR1.6:** Validar la correcta compilación del sistema en arquitectura RISC-V 64.

---

## 🚀 Verificación en la Shell

* **`ls`**: Devuelve los archivos disponibles leídos de la estructura del directorio de bloques:
  ```
  aetherv-shell> ls
  /readme.txt  /config.sys  /persist.dat
  ```
* **`cat /persist.dat`**: Solicita el archivo, lee el cluster 4 de la tarjeta de bloques y muestra su contenido:
  ```
  aetherv-shell> cat /persist.dat
  system_ok=1
  ```
