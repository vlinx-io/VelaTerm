## v0.1.101 — 2026-08-15

### Acceso remoto

- **Elija qué dirección usa el enlace para compartir: las direcciones de Tailscale ahora aparecen.** La lista de direcciones solo aceptaba los rangos IPv4 privados clásicos, por lo que las mallas VPN como Tailscale, que asignan direcciones del rango de NAT de operador (100.64.0.0/10), quedaban descartadas silenciosamente del panel de acceso remoto y del enlace de emparejamiento, aunque el servidor ya era accesible a través de ellas. Estas direcciones ahora se listan; los túneles VPN quedan al final para que nunca se conviertan en la opción predeterminada. Un nuevo selector de IP en el panel —visible antes de iniciar y con el servidor en marcha— muestra cada candidata con el nombre de su interfaz y marca los túneles VPN; al elegir una, su URL pasa al frente y el enlace de emparejamiento se regenera con exactamente ese host, de modo que el enlace copiado funciona en un dispositivo que solo alcanza esta máquina a través de la VPN, sin editar la URL a mano. Un código QR bajo el enlace de emparejamiento permite escanearlo directamente con el teléfono. La selección se recuerda; si la interfaz elegida desaparece, el panel vuelve a «Automático» sin olvidarla. El servidor en sí no cambia y sigue escuchando en todas las interfaces. Elegir una dirección que solo apareció después de iniciar el servidor —por ejemplo, una VPN conectada más tarde— ahora también actualiza de inmediato la URL copiada y el código QR, en lugar de solo el enlace de emparejamiento hasta el siguiente reinicio; los túneles VPN quedan detrás de las direcciones LAN en todas las plataformas, una dirección elegida con el servidor detenido determina el primer enlace de emparejamiento tras el arranque, y las regeneraciones de enlace solapadas ya no pueden sobrescribir un enlace más nuevo con uno más antiguo.

- **Compartir ahora sobrevive a un reinicio.** El token de emparejamiento se regeneraba cada vez que arrancaba el servidor, así que cerrar y volver a abrir VelaTerm invalidaba en silencio todos los enlaces compartidos y había que emparejar cada teléfono de nuevo. El token, los dispositivos emparejados y la lista de dispositivos bloqueados se guardan ahora en un archivo del directorio de datos legible solo por su propietario: un dispositivo ya emparejado se reconecta con su URL guardada tras un reinicio —la contraseña de acceso sigue siendo un segundo factor obligatorio— y un dispositivo revocado sigue revocado. VelaTerm también recuerda que el uso compartido estaba activo: si cierra la aplicación con el servidor en marcha, el siguiente arranque lo recupera en el mismo puerto, tanto en la aplicación de escritorio como en un servidor sin interfaz con `--serve`; si lo detiene usted mismo, nada arranca automáticamente. Si el arranque automático falla, por ejemplo porque el puerto está ocupado, la aplicación se inicia con normalidad y el panel de acceso remoto muestra el motivo. El campo de puerto recuerda ahora el puerto realmente usado en lugar de volver al valor predeterminado, y «Regenerar enlace» sigue siendo el interruptor de emergencia explícito: genera un token nuevo al instante, invalida todos los enlaces antiguos y sobrescribe el estado guardado. La contraseña de acceso nunca se escribe en el disco: solo se guarda un hash de memoria dura (Argon2id).

### Seguridad

- **Un dispositivo emparejado ya no puede administrar el propio uso compartido.** Cualquier navegador emparejado podía invocar los mismos comandos de administración que la aplicación de escritorio —crear un nuevo enlace de emparejamiento (lo que además vacía la lista de bloqueo de dispositivos), listar y revocar otros dispositivos, o detener y reconfigurar el servidor— y el almacén de ajustes entregaba a cada cliente el mapa completo de ajustes, incluido el hash de memoria dura de la contraseña de acceso y los ajustes de arranque automático que lee el siguiente inicio. Los comandos de administración quedan ahora reservados a la aplicación de escritorio y al shell de Electron; la API de ajustes filtra las claves de acceso remoto y el token de Gitea de cada lectura desde un dispositivo emparejado y rechaza las escrituras sobre ellas. Un dispositivo emparejado conserva aquello para lo que existe el emparejamiento —sus sesiones de terminal con acceso completo al shell—, pero ya no puede leer el verificador de la contraseña, invitar o expulsar a otros dispositivos ni redirigir el puerto que usará el próximo arranque. Los comandos que leen, escriben o eliminan secretos guardados — el token de Gitea y las contraseñas de host recordadas — también se rechazan ahora para un dispositivo emparejado, y los comandos que reciben rutas — leer, previsualizar, escribir, crear, renombrar y eliminar, así como mostrar el diff de git de un archivo o elegir la carpeta donde se clona un repositorio — resuelven primero los enlaces simbólicos y rechazan las rutas dentro del propio directorio de datos de VelaTerm, donde viven el estado de emparejamiento y las claves; cualquier otra ruta sigue funcionando, de modo que la exploración y edición remota de archivos permanecen intactas. Una prueba enumera cada comando remoto que acepta una ruta, de modo que un comando nuevo no puede saltarse esta comprobación sin ser detectado. Cuando una de estas protecciones rechaza una petición, el navegador muestra ahora un mensaje debidamente traducido en lugar de un error en inglés sin procesar.

- **Una revocación o un enlace regenerado sobrevive ahora también a la configuración de doble instancia.** En un servidor sin interfaz (`--serve`) con arranque automático activado, dos instancias del servidor mantenían cada una su propia copia del estado de emparejamiento guardado y lo reescribían completo: una revocación o un enlace nuevo hecho a través de una podía ser deshecho en silencio por la otra. Todas las instancias de un proceso comparten ahora un único estado de emparejamiento por directorio de datos: la revocación y la rotación surten efecto en todas partes de inmediato, y exactamente un escritor persiste el archivo, que sigue siendo la fuente de verdad entre reinicios reales.

- **Los inicios de sesión fallidos repetidos se frenan.** La comprobación de la contraseña de acceso usa Argon2id, caro a propósito, y puede intentarla cualquiera que alcance el puerto. Tras cinco intentos fallidos desde una dirección, los siguientes se rechazan durante un minuto antes de que se haga ningún trabajo de hash, y el propio hash se ejecuta ahora fuera del bucle de eventos del servidor con un tope estricto de verificaciones simultáneas: una avalancha de contraseñas erróneas ya no puede saturar el servidor con hashing de memoria dura ni ralentizarlo para los dispositivos ya conectados. El freno vive en memoria y se restablece con el servidor; el token de emparejamiento y la contraseña siguen siendo la barrera real. El límite ahora lo comparten todas las instancias del servidor que usan el mismo directorio de datos — la configuración de doble instancia con `--serve` ya no duplica el presupuesto de intentos — y cada intento se reserva antes de que empiece la comprobación de la contraseña, de modo que las peticiones paralelas desde una misma dirección no puedan colarse por debajo del límite. Un navegador frenado ve ahora un mensaje propio de límite de intentos en la pantalla de inicio de sesión en lugar de que se le diga que la contraseña era incorrecta; además, el freno ya no se recuerda como una contraseña incorrecta: pasada la pausa, el siguiente intento vuelve a procesarse sin recargar la página. Un intento abandonado a medias — la pestaña cerrada mientras la contraseña aún se comprobaba — libera ahora su reserva de inmediato en lugar de contar contra la dirección durante el resto del minuto, y un inicio de sesión correcto libera solo su propia reserva en vez de borrar todo el registro de la dirección: detrás de una dirección de red compartida, que alguien inicie sesión correctamente ya no restablece el presupuesto de intentos de un atacante, y los fallos registrados solo expiran con su minuto.

- **Los secretos en disco y en los registros se tratan con más cuidado.** El archivo con el estado de emparejamiento y la clave de cifrado de extremo a extremo se crean ahora legibles solo por el propietario desde el principio, en lugar de restringirse tras la primera escritura, y la base de datos de sesiones —que contiene el hash de la contraseña— también queda restringida al propietario. Un servidor sin interfaz (`--serve`) ya no imprime en los registros el secreto de larga duración del enlace de emparejamiento: si la salida no es un terminal, el enlace se retiene y se muestra una indicación en su lugar; `--print-pairing` vuelve a activarlo expresamente. El registro de dispositivos queda limitado a 32 entradas con nombres de longitud acotada, para que un cliente emparejado no pueda hacer crecer sin límite el archivo guardado, y si falla el guardado de una revocación o de un nuevo enlace, el error llega ahora a quien lo invocó en lugar de quedarse en una línea de registro. El arranque automático ya no sustituye a un servidor que ya se había iniciado a mano, y un error de arranque automático obsoleto desaparece en cuanto usted detiene el servidor.

### Correcciones

- **El emparejamiento ya se puede gestionar desde el shell de Electron.** Crear un enlace de emparejamiento, listar los dispositivos emparejados y revocar un dispositivo solo existían como comandos de escritorio (Tauri); el despachador WebSocket que usan el shell de Electron y los clientes de navegador respondía «Unknown command», dejando inservible el panel de acceso remoto en ese entorno. Los tres comandos pasan ahora por las mismas funciones centrales en ambos transportes, de modo que no pueden divergir, y pruebas de regresión cubren las nuevas rutas de despacho, incluida la creación de un enlace de emparejamiento real contra un servidor local en ejecución.

## v0.1.100 — 2026-08-10

### Agentes de IA

- **Kiro CLI pasa a ser un tipo de sesión de primera clase.** Las sesiones de Kiro tienen su propio nodo en el árbol, un indicador de estado autoritativo de «trabajando» o «en espera» que se apoya en los propios lifecycle hooks de Kiro, notificaciones al terminar un turno, reanudación automática de la misma conversación al volver a abrir el nodo, argumentos de inicio y una opción para omitir confirmaciones, además del lanzamiento mediante vspawn: todo lo que los demás agentes ya tenían. VelaTerm clona tu agente Kiro predeterminado en un agente `vlx-term` propio, añade a esa copia lifecycle hooks de solo observación y lanza esa copia; tu archivo de agente no se modifica nunca, y tu prompt, tus herramientas y tus servidores MCP se conservan sin cambios. Kiro no tiene ningún hook de solicitud de permisos, por lo que el indicador se mantiene en «trabajando» mientras espera tu aprobación.

### Correcciones

- **Los programas iniciados desde el terminal ya no heredan el entorno propio del AppImage (Linux).** El lanzador del AppImage apunta `PYTHONHOME`, `PYTHONPATH`, `PERLLIB`, `QT_PLUGIN_PATH` y las rutas de plugins de GStreamer al directorio de montaje temporal del paquete, y coloca los directorios del paquete por delante de todo lo demás en `PATH` y `LD_LIBRARY_PATH`. Un terminal entrega todo su entorno al shell que inicia, así que el `python3` del sistema buscaba su biblioteca estándar dentro del paquete y se negaba a arrancar, y otros programas enlazados dinámicamente cargaban la copia de una biblioteca incluida en el paquete en lugar de la del sistema. Ahora VelaTerm elimina esas rutas del paquete antes de iniciar un shell o una herramienta externa, y no toca los valores que hayas definido tú. `APPDIR` y `APPIMAGE` siguen visibles, de modo que los programas que comprueban si se están ejecutando desde un AppImage siguen obteniendo su respuesta. Solo afectaba a las compilaciones AppImage; el paquete deb, macOS y Windows se comportan igual que antes.

## v0.1.99 — 2026-08-09

### Terminal

- **Shift+Intro escribe un salto de línea en lugar de enviar.** Los terminales no tienen codificación para Intro con una tecla modificadora, así que las CLI de agentes como Claude Code y Codex solo recibían un retorno de carro normal y enviaban el mensaje cuando aún se estaba escribiendo. Ahora VelaTerm emite ESC+CR, la misma secuencia que esas herramientas esperan de una asignación de teclas de iTerm2, de modo que la entrada de varias líneas ya funciona, también en macOS, donde el manejador de teclas personalizado ni siquiera se instalaba. La composición en un método de entrada no se altera: Intro sigue confirmando el candidato.

### Proyectos y organización

- **Actualizar el estado de una sola sesión.** En un panel con filtro de estado, las sesiones incorporan la acción «Actualizar estado», que reevalúa únicamente esa sesión según las condiciones del propio panel y la añade o la quita mientras las demás permanecen en su sitio. La acción pertenece al panel desde el que se abrió el menú, por lo que las divisiones anidadas nunca toman prestado el filtro de otro panel. El resultado se guarda por panel y se restaura tras reiniciar.
- **Quitar una marca cuesta un clic.** Elegir el emoji ya aplicado lo elimina, así que la entrada específica para quitarlo y su separador desaparecen. La insignia de emoji del botón de filtro también se retira: el resaltado ya indica que hay un filtro de marca activo y el menú indica cuál es.

### Correcciones

- **La integración de escritorio del AppImage de Linux se instala en cualquier equipo.** El icono incluido era un enlace simbólico a una ruta absoluta de la máquina de compilación, por lo que herramientas como Gear Lever y AppImageLauncher no podían extraerlo, aunque la aplicación funcionara con normalidad. Ahora el enlace es relativo. También se corrigió el requisito de glibc a 2.35 tras medir las bibliotecas incluidas y no solo el ejecutable, lo que convierte a Ubuntu 22.04 en la distribución más antigua compatible con la aplicación de escritorio.

## v0.1.98 — 2026-08-02

### Agentes de IA

- **Grok Build se incorpora a VelaTerm como agente de primera clase.** Instala, inicia y reanuda Grok 4.5 con identificadores de sesión estables, lifecycle hooks oficiales, estados precisos de trabajo y permisos, transcripciones unificadas, detalles de uso y un icono oficial que se adapta al tema en las vistas de escritorio, navegador y móvil.

### Proyectos y organización

- **Divide la barra lateral de proyectos en vistas de trabajo independientes.** Cualquier panel del árbol puede volver a dividirse hacia abajo y recupera tras reiniciar su propia búsqueda, filtros de estado y emoji, estado de plegado y proporción de tamaño. Todos los paneles siguen siendo proyecciones del mismo árbol de proyectos gestionado por el backend, por lo que los cambios se sincronizan sin duplicar datos de negocio.
- **Marca y filtra nodos sin perder el contexto.** Los proyectos, grupos y sesiones pueden llevar marcadores emoji. Un contenedor marcado conserva todo su subárbol, la pertenencia a estados permanece estable mientras trabajas, están disponibles tanto la incorporación dinámica como la actualización manual, y las condiciones de estado y emoji se combinan como una unión.
- **Crea un proyecto vacío al instante.** Elige el directorio superior, valida el nombre y crea e importa la carpeta en un único flujo. Si se produce un fallo parcial, solo se reintenta la importación, sin crear directorios duplicados.

### Interfaz

- **Comparte VelaTerm donde esté tu comunidad.** El diálogo de compartir ahora incluye WeChat Moments, Weibo, Xiaohongshu, X, Reddit, Hacker News, LinkedIn, Facebook, Telegram y WhatsApp, con un flujo de código QR para WeChat y una invitación a compartir en el diálogo de actualización.
- **Interacciones pequeñas, pero más cuidadas.** Las pestañas de terminal temporales se pueden renombrar antes de convertirse en sesiones guardadas. Los campos de entrada normales desactivan las mayúsculas automáticas de los teclados móviles sin alterar la entrada del terminal.

## v0.1.97 — 2026-07-25

### Agentes de IA

- **Las sesiones ya no se quedan atascadas en «trabajando».** Codex informaba de la actividad de herramientas y del fin de turno mediante procesos efímeros distintos, cuyos callbacks podían llegar desordenados y dejar un turno terminado mostrado como aún en curso. Ahora se descartan los informes intermedios que llegan después del final de su propio turno, y un nuevo enlace de fin de sesión cubre las sesiones que terminan sin evento de finalización.
- **Los turnos interrumpidos se resuelven en segundos.** Pulsar Esc, o un error de flujo, termina un turno de Claude o Codex sin ningún callback de finalización. Seis segundos de silencio en el terminal corrigen ahora esa sesión a en espera de forma discreta, sin lanzar una notificación de «ha respondido».

### Interfaz

- **Atajos de división fiables en macOS.** Dividir a la derecha (Cmd+D) y dividir hacia abajo (Cmd+Shift+D) se registran ahora como comandos del menú Terminal nativo, de modo que macOS ya no intercepta la combinación antes que VelaTerm.
- **Un guardado por pulsación.** Cmd+S se procesaba tanto en el atajo global como en el editor enfocado, lo que podía escribir el mismo archivo dos veces en una sola pulsación.

## v0.1.96 — 2026-07-23

### Agentes de IA

- **El estado de Codex confía en lifecycle hooks, no en suposiciones del terminal.** Las sesiones modernas de Codex usan únicamente los lifecycle hooks oficiales como fuente de actividad. Un enlace `SessionStart` verifica la integración, la ausencia de callbacks se muestra como «Estado no disponible» y el texto o la actividad del terminal ya no puede sobrescribir los estados de trabajo, confirmación o finalización.
- **Uso de Codex más actualizado tras cada turno.** El panel Info muestra de inmediato el snapshot rollout local, lo concilia con los límites en vivo, vuelve a actualizar después de que Codex escriba el snapshot token final e ignora respuestas tardías de una sesión anterior.

### Interfaz

- **Selección fiable en el árbol de proyectos de macOS.** Las filas virtuales ya no dependen de transform del compositor, lo que evita que coordenadas de hit-test obsoletas de WKWebView envíen acciones de pasar el cursor, hacer clic o arrastrar a otra fila después de desplazarse o actualizar el árbol.

## v0.1.95 — 2026-07-21

### Agentes de IA

- **Kimi Code y Zoo Code llegan al árbol de sesiones.** VelaTerm ya puede iniciar, reanudar, instalar y configurar ambos agentes. Kimi usa sus lifecycle hooks oficiales para informar de forma autoritativa los estados de trabajo, permiso y espera; Zoo Code conserva una identidad de tarea estable y usa detección del terminal cuando no hay hooks externos.
- **Actualización en vivo del uso de Codex.** El panel Info consulta el Codex app server para obtener los límites actuales y mantiene la instantánea rollout local como alternativa compatible.

### Proyectos y terminales

- **Abre proyectos con `vela <path>`.** Las versiones empaquetadas pueden instalar un comando shell al estilo de VS Code. Una segunda llamada envía el proyecto a la ventana VelaTerm existente en lugar de abrir una instancia duplicada.
- **Clonado Git visible y cancelable.** Clone Project muestra etapas, porcentaje y tiempo transcurrido, avisa si el progreso se detiene y puede cancelar todo el árbol de procesos Git sin dejar un destino incompleto. Las credenciales y query tokens se ocultan en errores y registros de auditoría.
- **Terminales WSL en Windows.** Todas las distribuciones WSL instaladas se ofrecen junto a PowerShell, cmd y Git Bash para terminales normales. Los agentes siguen usando el shell anfitrión de Windows para mantener fiables los hooks y las rutas de ejecutables.

### Interfaz y fiabilidad

- **Control más claro de sesiones en segundo plano.** Los menús muestran el estado en vivo de cada sesión y el diálogo de límite permite cerrar varias pestañas seleccionadas a la vez.
- **Ciclo de vida más seguro y notas multilingües.** Se confirma antes de detener sesiones activas; la identidad lifecycle exacta de Codex prevalece sobre análisis rollout ambiguos; las notas de actualización admiten todos los idiomas incluidos.

## v0.1.94 — 2026-07-12

### Localización

- **Interfaz en vietnamita.** Tiếng Việt ya está disponible en el selector de idiomas y se selecciona automáticamente cuando el sistema usa una configuración regional vietnamita.

### Navegador

- **Inicio más rápido del navegador integrado.** Cada pestaña del navegador dispone ahora de accesos directos de un clic para ChatGPT, Claude, Gemini y Google. Los menús contextuales de proyectos y grupos también permiten crear una página permanente del navegador directamente en la parte correspondiente del árbol de sesiones.

### Imágenes y documentos

- **Pegado fiable de rutas de imagen en macOS.** Cuando WebKit no expone una imagen copiada como archivo, VelaTerm la lee ahora del portapapeles nativo y la sigue subiendo como ruta de archivo, en lugar de recurrir silenciosamente al marcador de imagen nativo del agente. Las ventanas remotas siempre muestran el ajuste de pegado de imágenes, explican por qué se requiere el modo de ruta de archivo y desactivan la opción nativa no disponible.
- **Pegado de imágenes en documentos fuente.** El editor de código fuente acepta ahora imágenes del portapapeles. Los documentos Markdown guardados las almacenan junto al documento en `assets/` e insertan una sintaxis de imagen Markdown portable; los borradores sin guardar incrustan los datos de la imagen para que no se pierdan al limpiar los archivos temporales.

### Interfaz

- **Los menús contextuales permanecen visibles y apuntan al elemento correcto.** Los menús abiertos cerca del borde derecho se miden y desplazan correctamente. Al hacer clic con el botón derecho en un nodo del árbol, ahora solo se resalta el objetivo del menú sin cambiar la selección existente; los menús de grupo también incluyen un terminal limitado a ese grupo.
- **Edición y etiquetas de estado más limpias.** El texto fuente ya no representa ligaduras tipográficas en forma de flecha para secuencias como comentarios HTML, los porcentajes de uso se etiquetan explícitamente como utilizados y el menú contextual nativo no relacionado del WebView anfitrión ya no aparece detrás de los menús de VelaTerm.

### Correcciones

- **Codex permanece en el historial normal del terminal.** Las sesiones de Codex iniciadas por VelaTerm ahora utilizan el modo de terminal en línea. Por tanto, pulsar Esc para interrumpir o retroceder ya no cambia los búferes de pantalla del terminal ni desplaza la vista del historial hasta arriba. La configuración de Codex del usuario no se modifica.
