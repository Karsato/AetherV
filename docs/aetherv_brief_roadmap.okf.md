path/to/docs/aetherv_brief_roadmap.okf.md
```

---

# Roadmap AetherV OS

## Fase 1: Base de la Arquitectura y Kernel

- **Objetivo:** Crear una arquitectura monolítica básica con kernel básico.
- **Contenido:**
  - Desarrollo del núcleo en S-Mode (RISC-V).
  - Implementación de llamadas al sistema (`ecall`).
  - Mapeo de memoria y gestión de tareas.

## Fase 2: Multitasking & User Space

- **Objetivo:** Introducir hilos del kernel y aislamiento de espacio de usuario.
- **Contenido:**
  - Planificador Round-Robin preventivo.
  - Implementación de llamadas al sistema para controlar tareas (`sys_yield`, `sys_exit`).
  - Integración con interrupciones de temporizador.

## Fase 3: Hardware Drivers & I/O Subsystem

- **Objetivo:** Desarrollar drivers de hardware y subsistema de E/S.
- **Contenido:**
  - Mapeo MMIO y inicialización de dispositivos VirtIO (GPU, Input).
  - Implementación de colas de descriptores y protocolos VirtIO.

## Fase 4: Modern Microkernel Evolution

- **Objetivo:** Transición al microkernel minimalista con aislamiento en U-Mode.
- **Contenido:**
  - Aislamiento de ejecución en U-Mode.
  - Paso de mensajes IPC de alta velocidad (Rendezvous y Notificaciones).
  - Migración de drivers a procesos de usuario.

## Fase 5: Servidor de Ventanas & Sistema de Archivos Virtual

- **Objetivo:** Implementar un servidor de ventanas y sistema de archivos virtual.
- **Contenido:**
  - Desarrollo de una interfaz gráfica simple.
  - Integración con un micro-servidor de archivos.

## Fase 6: User shell & Sistema Operativo Completo

- **Objetivo:** Implementar la consola interactiva y sistema operativo completo.
- **Contenido:**
  - Desarrollo de una interfaz gráfica completa.
  - Integración con un servidor web para administración.

## Fase 7: Mejoras y Optimizaciones

- **Objetivo:** Mejorar la eficiencia, escalabilidad y seguridad del sistema.
- **Contenido:**
  - Optimización de código y algoritmos.
  - Implementación de mecanismos de failover y recuperación.

---

Este roadmap proporciona una visión general de los objetivos y contenido esperado para cada fase del proyecto AetherV OS.