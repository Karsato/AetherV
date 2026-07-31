---
id: "aetherv-phase10-net-server-2026-07-31"
title: "AetherV OS - Fase 10: Servidor de Red en Espacio de Usuario (U-Mode Net Server)"
type: knowledge-note
source: "AetherV OS Development"
author: "Antigravity Coding Assistant"
date_created: 2026-07-31
last_modified: 2026-07-31
tags:
  - topic/os-development
  - topic/riscv
  - topic/net-server
  - topic/microkernel
topics:
  - "AetherV OS"
  - "U-Mode Net Task"
  - "VirtIO-Net Driver Discovery"
---

# AetherV OS - Fase 10: Servidor de Red & Monitoreo HTTP en Modo Usuario

## 🎯 Resumen Ejecutivo
Implementación de la **Fase 10: Servidor de Red (Net Server)** ejecutándose enteramente en espacio de usuario (**U-Mode / Ring 3**) en AetherV OS. Este módulo introduce capacidades básicas de descubrimiento de hardware de red virtual VirtIO y un servidor de monitoreo HTTP simulado para reportar estadísticas críticas del sistema mediante paso de mensajes (IPC) síncronos.

Adicionalmente, se integra el soporte del cliente en la consola interactiva (`shell_task`) mediante comandos de red (`netstat` y `curl`) para consultar las métricas e interactuar con el servidor de red.

---

## 🔑 Conceptos Clave & Arquitectura de Red

```
   ┌─────────────────────────────────────────────────────────┐
   │                       Shell Task                        │
   │                     (U-Mode Task 9)                     │
   └────────────┬─────────────────────────────┬──────────────┘
                │                             │
       netstat (IPC Status)            curl (IPC HTTP Get)
                │                             │
                ▼                             ▼
   ┌─────────────────────────────────────────────────────────┐
   │                      Net Server                         │
   │                    (U-Mode Task 10)                     │
   └────────────────────────────┬────────────────────────────┘
                                │
                  VirtIO-Net Discovery (MMIO Scan)
                                │
                                ▼
                   [ VirtIO Network Hardware ]
```

1. **Descubrimiento de Red (VirtIO-Net)**:
   * El driver realiza un escaneo dinámico de la memoria MMIO en el espacio reservado de VirtIO (`0x10001000`–`0x10009000`) buscando el Device ID `1` (que identifica a las tarjetas de red de VirtIO).
   * Mantiene un estado de contingencia híbrido: si el dispositivo físico de red no es expuesto por QEMU, el servidor de red se inicia automáticamente en **Modo Simulado** para garantizar el funcionamiento continuo del sistema sin provocar kernel panics.

2. **Protocolo IPC de Red**:
   * **`NET_CMD_STATUS` (4001)**: Solicita información de la dirección IP asignada, estadísticas de paquetes recibidos (RX) / enviados (TX) y el estatus de inicialización del driver de hardware.
   * **`NET_CMD_GET_HTTP` (4002)**: Solicita contenido de monitoreo del servidor web HTTP en formato de texto (ej. estadísticas del sistema).

---

## 🛠️ Objetivos & Resultados Clave (OKRs) de la Fase 10

- [x] **KR1.1:** Desarrollar `net_server_task` en espacio de usuario e integrarlo en la tabla de tareas del microkernel (ID de tarea 10).
- [x] **KR1.2:** Implementar escaneo dinámico del bus VirtIO MMIO buscando tarjetas de red (Device ID 1).
- [x] **KR1.3:** Añadir soporte IPC para obtención de estado de red (`NET_CMD_STATUS`) y peticiones HTTP simuladas (`NET_CMD_GET_HTTP`).
- [x] **KR1.4:** Incorporar los comandos `netstat` y `curl` en la consola interactiva (`shell_task`) para realizar solicitudes a la pila de red en tiempo real.
- [x] **KR1.5:** Garantizar una compilación exitosa y libre de fallos en la arquitectura RISC-V 64.

---

## 🚀 Verificación de Comandos en la Shell

* **`netstat`**: Devuelve la configuración de red actual:
  ```
  aetherv-shell> netstat
  IP: 192.168.1.10 | RX/TX: Active | Dev: Simulated
  ```
* **`curl`**: Realiza una petición de monitoreo HTTP simulada y renderiza la cabecera e info devuelta por el servidor de red:
  ```
  aetherv-shell> curl
  HTTP/1.1 200 OK\nSrv: AetherV-Web
  ```
