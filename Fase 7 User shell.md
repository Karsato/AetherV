¡Entendido! La **Fase 7** encaja de forma natural justo después de implementar el Sistema de Archivos Virtual (Fase 6). Al ejecutarse en su propia rama dedicada (`feature/07-user-shell`), esta fase consolidará toda la arquitectura del microkernel permitiendo al usuario interactuar en tiempo real con el sistema a través de comandos.

A continuación tienes la especificación completa para incorporar formalmente esta fase a la documentación de **AetherV OS**:

## 🐚 Fase 7: Interactive User Shell (`feature/07-user-shell`)

### 🎯 Resumen Ejecutivo

Implementación de un intérprete de comandos (Shell) interactivo ejecutado íntegramente en espacio de usuario (**U-Mode / Ring 3**). La Shell actuará como un cliente IPC de alto nivel que coordinará las peticiones del usuario resolviendo servicios mediante el **Nameserver** y comunicándose con el controlador de teclado/UART, el servidor gráfico (`gpu_driver_server`) y el sistema de archivos (`vfs_server`).

### 🔑 Conceptos Clave

- **User-Space Shell:** A diferencia de los kernels monolíticos donde la consola vive en el espacio de núcleo, en AetherV OS la Shell es un proceso aislado que no posee privilegios especiales.
    
- **IPC Command Dispatcher:** La Shell decodifica la entrada de texto y traduce las órdenes en mensajes IPC dirigidos a los servidores de servicio específicos registrados en el Name Server.
    
- **Standard I/O Redirection (U-Mode):** Consumo de flujos de entrada (`stdin` vía teclado/UART) y salida (`stdout` vía terminal/GPU) mediante paso de mensajes IPC.
    

### 💡 Especificación de Diseño y Tareas Clave

#### 1. Módulo Servidor de Shell (`src/bin/shell.rs` o `shell_task` en U-Mode)

- **Punto de Entrada U-Mode:** Creación del proceso cliente en espacio usuario con permisos `PTE_U`.
    
- **Bucle REPL (Read-Eval-Print Loop):**
    
    1. **Read:** Escucha asíncrona/síncrona de eventos de caracteres leídos desde el driver de teclado (VirtIO Input) o UART mediante `sys_ipc_recv`.
        
    2. **Eval:** Análisis sintáctico (_parsing_) de la cadena de texto introducida (separación de comando y argumentos).
        
    3. **Print / Dispatch:** Envío del comando por IPC al servidor correspondiente y renderizado del resultado en pantalla.
        

#### 2. Comandos Base a Implementar

- `help`: Muestra la lista de comandos disponibles y servicios activos en el sistema.
    
- `ps`: Consulta al planificador/Nameserver para listar los procesos y servicios en ejecución y sus Task IDs.
    
- `clear`: Envía un mensaje IPC al servidor gráfico (`gpu_driver_server`) para limpiar el Framebuffer o la consola.
    
- `echo <texto>`: Imprime una cadena en la salida estándar.
    
- `ls` / `cat <archivo>`: (_Integración con Fase 6_) Envía peticiones de lectura al `vfs_server` para inspeccionar el sistema de archivos.
    
- `color <hex>`: Demostración IPC que envía un mensaje al `gpu_driver_server` para cambiar el color de fondo de la interfaz gráfica.
    

### 🛠️ Acciones / Plan de Implementación (`feature/07-user-shell`)

- [ ] **Paso 1: Creación de la rama Git**
    
    Bash
    
    ```
    git checkout -b feature/07-user-shell
    ```
    
- [ ] **Paso 2: Resolución de Servicios**
    
    - Consultar al `nameserver` (ID de servicio conocido) para obtener los `task_id` de los servidores de entrada (`input`), pantalla (`display`) y archivos (`vfs`).
        
- [ ] **Paso 3: Buffer de Entrada de Línea**
    
    - Implementar un buffer de caracteres para gestionar la tecla de borrado (_backspace_), salto de línea (_enter_) y eco de caracteres en tiempo real.
        
- [ ] **Paso 4: Parser y Despachador de Mensajes**
    
    - Diseñar la estructura `ShellCommand` para interpretar argumentos.
        
    - Mapear comandos a llamadas `sys_ipc_send` / `sys_ipc_reply_recv`.
        
- [ ] **Paso 5: Validación en QEMU**
    
    - Probar la introducción de comandos mediante el teclado del portátil en la ventana gráfica de QEMU y verificar la respuesta de los servidores U-Mode.