## v0.1.104 — 2026-08-25

### Agentes de IA

- **La tarjeta de tarea hija ofrece ahora los modelos reales de cada agente, y el indicador de esfuerzo que ese agente entiende de verdad.** La tarjeta construía sus argumentos de inicio con `--model` y `--effort` para todos, pero solo Claude, Kiro y Antigravity nombran así el esfuerzo de razonamiento: Grok y Zoo lo llaman `--reasoning-effort`, y Cline lo llama `--thinking`. Elegir un nivel de esfuerzo para cualquiera de los demás le pasaba a la CLI un indicador que nunca había oído, y la sesión no llegaba a arrancar. Ahora cada agente aporta sus propios nombres de indicador y sus propios valores. El control de modelo sigue lo que cada CLI puede contarnos: a las que saben listar su catálogo (OpenCode, Grok, Crush, Antigravity, Cursor, pi, Kiro) se les pregunta y ofrecen la lista real; las que tienen un conjunto fijo (Claude, Codex, Kimi Code) ofrecen ese conjunto; y el resto te dan un campo de texto con un ejemplo del formato que esperan. Un agente que no esté instalado o en el que no hayas iniciado sesión lo dice, en lugar de quedarse girando para siempre. Elegir «Predeterminado» borra ahora el valor heredado en lugar de dejar el anterior en su sitio, y elegir un nivel de esfuerzo ya no descarta el modelo heredado de la sesión padre.

- **Una tarea hija creada desde una sesión de Kimi Code sigue siendo Kimi Code.** Kimi Code faltaba en la lista que usa el camino de creación para heredar el agente del padre, así que sus sesiones hijas volvían en silencio con el agente predeterminado.

### Espacio de trabajo

- **Un grupo puede moverse a un worktree después de haberlo creado.** El worktree se elegía al crear el grupo y quedaba fijado desde entonces; cambiar de idea obligaba a borrar el grupo y volver a construirlo. Haz clic derecho sobre un grupo y elige «Move to Worktree…» para crear un worktree nuevo, vincular uno existente o reapuntar un grupo que ya esté vinculado. Solo cambia el grupo en sí: las sesiones que ya están dentro conservan el directorio con el que se crearon —una sesión en marcha no puede moverse a otro directorio por debajo de sí misma—, mientras que las sesiones creadas después arrancan en el worktree.

- **El modo espejo abarca ahora todo el árbol de la barra lateral.** Compartía la selección y los paneles plegados; el cuadro de búsqueda y los filtros de estado y de marcador se quedaban en local, bajo la idea de que sincronizarlos interrumpe a quien está buscando algo. Ese razonamiento estaba al revés: reflejar significa que las dos ventanas mantienen el mismo estado, no que una repita las pulsaciones de la otra —un filtro que está activo aquí lo está allí—. Lo que de verdad interrumpe a la gente es que los dos lados muestren árboles distintos. Ahora viaja toda proyección de la barra lateral: la disposición dividida, el nombre de cada proyección, su texto de búsqueda, sus filtros de estado y de marcador, y su propio estado de plegado. El formato de la instantánea pasa a la versión 2, y un cliente que ejecute una versión anterior deja de reflejar en lugar de aplicar medio fotograma, así que recarga cualquier ventana que hayas dejado abierta durante la actualización.

### Interfaz

- **Cerrar la ventana en macOS hace la misma pregunta que salir.** ⌘Q y la entrada del menú pasaban por la confirmación propia de la aplicación, pero el botón rojo de cerrar destruía la ventana sin más — y esa ventana contiene la webview donde vive el diálogo de confirmación. O no obtenías ninguna confirmación, o te salía el respaldo nativo reducido, sin la casilla de «guardar el espacio de trabajo» y con el texto sin traducir. Las tres plataformas mantienen ahora la ventana abierta hasta que respondes.

- **«Guardar el espacio de trabajo» viene marcado de serie, y se queda donde tú lo dejes.** Perder una disposición cuesta más que una instantánea que no querías, así que la casilla arranca marcada. Además se le olvidaba: el ajuste se escribía en la base de datos con un retardo de 400 ms, y salir mataba el proceso dentro de esa ventana, así que el siguiente arranque se reconciliaba con el valor antiguo y deshacía tu cambio. Ahora la escritura se vuelca antes de salir, con un techo de 600 ms para que un backend atascado no pueda dejar el botón de confirmar girando. (Aportado por FarhadGSRX.)

- **Las contraseñas del panel de conexión remota pueden mostrarse.** Tanto la contraseña de la URL como la de SSH tienen un botón con forma de ojo que alterna entre texto oculto y texto plano. El estado de mostrado es local al campo y se restablece al cerrar el panel, así que nunca queda una contraseña a la vista en pantalla.

- **El distintivo de filtros de la barra lateral cuenta todos los filtros activos.** Un filtro de marcador solo encendía el botón sin decir nada más, así que el distintivo podía marcar 1 con dos filtros activos. Ahora suma estados y marcadores, y coincide con las marcas del desplegable; un único filtro de estado conserva su punto de color.

### Correcciones

- **Las actualizaciones automáticas en macOS vuelven a funcionar.** Los paquetes de la v0.1.103 llevaban entradas acompañantes de AppleDouble (`._VelaTerm.app`), a las que el actualizador les quita el primer componente de la ruta —dejándola vacía—, y entonces se negaba a desempaquetar el archivo. Afectaba a las dos arquitecturas, así que cualquier usuario de macOS con la v0.1.103 se quedaba atascado ahí. El empaquetado ya no escribe esas entradas.

- **Los controles nativos siguen el tema de la aplicación cuando difiere del del sistema.** Aplicar un tema cambiaba los colores propios de la aplicación, pero nunca actualizaba `color-scheme`, que se fijaba una sola vez al arrancar a partir de la preferencia del sistema y no volvía a cambiar, así que las casillas, los desplegables y las barras de desplazamiento seguían en oscuro bajo una aplicación clara en un sistema oscuro. (Aportado por FarhadGSRX.)

- **Un clon recién hecho vuelve a compilar.** El crate de Rust incrusta `../dist` en tiempo de compilación, y el comando de desarrollo no lo genera, así que un repositorio recién clonado fallaba al compilar antes siquiera de poder ejecutarse. El script de compilación crea ahora ese directorio cuando falta. (Aportado por FarhadGSRX.)

---

## v0.1.103 — 2026-08-24

### Correcciones

- **Las sesiones de Codex en Windows ya no se niegan a iniciar.** Cada sesión de Codex fallaba inmediatamente con `unexpected argument '--codex-hook'` porque la tabla TOML de los hooks de ciclo de vida se pasaba por línea de comandos con espacios y comillas dobles, y `codex.cmd` instalado por npm reprocesa eso a través de cmd.exe, que elimina las comillas y divide el valor en múltiples argumentos. Ahora se omite la inyección de hooks en Windows; la detección de estado recurre a las heurísticas existentes de notify / screen / busy, que siguen informando estados de reposo y actividad, aunque con menos precisión que los hooks. macOS y Linux no se ven afectados y continúan usando hooks.

- **Se revirtió la corrección del pre-editado IME en Windows de v0.1.102.** La corrección que restauró la superposición de composición para la entrada en chino, japonés y coreano también le añadió color de fondo, borde de 1px y bordes redondeados, lo que dibujaba un pequeño recuadro alrededor del texto de pre-edición dentro de la terminal — algo que no debería aparecer ahí. Como el dimensionamiento de la superposición, la geometría del contenedor auxiliar y la limpieza del textarea eran interdependientes, fue necesario revertir todo el cambio. El problema subyacente — escritura a ciegas de CJK en Windows — sigue abierto y se rastrea en el issue #6.

---

## v0.1.102 — 2026-08-23

### Agentes de IA

- **Preajustes de agente: varias CLI compatibles conviviendo.** Cada tipo de agente estaba atado a un único ejecutable, así que un fork, una compilación nocturna o una segunda CLI que habla el mismo protocolo no tenían por dónde entrar: había que editar los argumentos de inicio de un tipo existente y se perdía el original. Un preajuste define ahora su propio ejecutable, su propio icono y sus propios argumentos de inicio, y aparece en el menú de nueva sesión junto a los tipos integrados. Las sesiones registran con qué preajuste se crearon, de modo que bifurcar una conserva el mismo ejecutable, y un preajuste creado en el escritorio aparece también en los navegadores emparejados y en los clientes remotos, icono incluido, porque el icono viaja como datos y no como una ruta de una máquina concreta. Las sesiones existentes no se tocan: una base de datos de una versión anterior arranca exactamente igual que antes.

- **La tarjeta de tarea hija elige el modelo y el nivel de esfuerzo, y una sola respuesta lo resuelve en todas partes.** Cuando un agente pide lanzar una tarea hija, la tarjeta de confirmación ofrece ahora el modelo y —cuando el agente lo admite— el esfuerzo de razonamiento, rellenados a partir de los argumentos de inicio del propio padre, de modo que el caso habitual se resuelve con un solo clic. Cambiar la tarjeta a otro agente vuelve a deducir ambos valores, así que el nombre de un modelo de una CLI ya no puede acabar en la línea de comandos de otra. La tarjeta aparece en todos los clientes conectados y responderla en uno la retira ahora de los demás; la primera respuesta además reclama la tarea en el servidor, de modo que confirmar en el teléfono y en el escritorio dentro del mismo segundo crea un único worktree y una única sesión hija en lugar de dos.

### Espacio de trabajo

- **Modo espejo: una misma disposición en todos los clientes.** El flujo del terminal siempre se compartió —un PTY, un flujo de bytes—, pero la disposición que lo rodea vivía solo en el almacenamiento del navegador de cada cliente, así que un navegador abierto en la LAN mostraba sus propias pestañas y divisiones, y reorganizar una pantalla no afectaba en nada a la otra. Con el modo espejo activo, las pestañas, las divisiones, la sesión activa, la selección de la barra lateral y los paneles plegados se publican a todos los clientes y todos los siguen. El anfitrión controla el interruptor desde el panel de acceso remoto. Reorganizar en cualquiera de los dos lados surte efecto en el otro; una sesión que sale de la disposición de esta ventana se desacopla en lugar de cerrarse, de modo que seguir a otro cliente nunca termina el proceso de nadie; y aplicar la disposición de otro cliente no le quita el teclado a quien está escribiendo en local. Los teléfonos quedan fuera: la navegación de dos niveles del teléfono es una interfaz de otra forma, y copiar en ella un árbol de divisiones de escritorio no le sirve a nadie.

- **La pestaña Git de la barra lateral derecha se ha convertido en un cliente Git utilizable.** Antes solo listaba los archivos modificados. Ahora añade al área de preparación archivos sueltos o grupos enteros y los quita de ella, descarta cambios, escribe un commit (con la opción de enmendar) y muestra el historial con los archivos y los diffs de cada commit, agrupados en secciones plegables de preparados, modificados, sin seguimiento y confirmados. Las rutas se manejan desde la raíz del repositorio, así que una sesión abierta en un subdirectorio actúa sobre los archivos que dice, y un HEAD desacoplado se indica como tal en lugar de mostrar una rama llamada HEAD.

### Interfaz

- **⌘Q hace ahora la misma pregunta que cerrar la ventana.** La entrada «Salir» del menú de la aplicación era la del sistema, que termina el proceso sin más: pulsar ⌘Q se saltaba la confirmación de «guardar el espacio de trabajo» que sí muestra el botón de cerrar, así que la misma intención se comportaba de forma distinta según cómo la expresaras. Ambos caminos pasan ahora por una única confirmación. Si la ventana que la muestra se ha recargado o ha fallado mientras tanto, volver a pulsar ⌘Q repite la pregunta y recurre a un diálogo nativo, en lugar de dejar la aplicación sin manera de salir.

- **Las indicaciones de atajos muestran las teclas que de verdad funcionan.** Los valores predeterminados cambian según la plataforma, y un navegador se reserva para sí las combinaciones de ⌘/Ctrl con letras —⌘D guarda un marcador, ⌘T abre una pestaña—, así que en macOS los atajos con ⌘ de la propia aplicación nunca llegaban a la página cuando VelaTerm se abría como una URL. Las pestañas de navegador normales usan ahora las combinaciones con Ctrl+Alt en todos los sistemas operativos, mientras que la aplicación de escritorio y las ventanas de conexión remota conservan ⌘. Las ayudas emergentes y la indicación de la pestaña vacía muestran la combinación que esté en vigor, incluida una que hayas reasignado tú, en lugar de una combinación con ⌘ fija en el código; y el terminal bloquea exactamente las combinaciones que la aplicación ha reclamado, de modo que reasignar una acción se lleva esa tecla consigo.

- **Los preajustes de fuente incluyen Nerd Fonts y CJK, y una fuente personalizada que no esté instalada lo dice.** La lista de preajustes incorpora las familias Nerd Font y CJK más habituales, y una fuente escrita a mano se muestra de vuelta y se comprueba: si el sistema no la tiene, la página de ajustes lo indica en lugar de recurrir en silencio a una predeterminada que no se parece en nada a la que pediste.

- **Los campos de texto en macOS ya no ponen mayúsculas ni corrigen lo que escribes.** Las mayúsculas automáticas, la autocorrección y el corrector ortográfico del sistema se aplicaban a todos los campos de la aplicación, incluidos los nombres de sesión y los campos de comandos, donde «npm» se convertía en «Npm». Ahora están desactivados en todas partes.

### Windows

- **Al escribir en chino, japonés o coreano vuelven a verse el texto en composición y la ventana de candidatos.** Los dos eran invisibles: escribías a ciegas y solo veías el resultado tras pulsar Intro. La culpa era de dos reglas CSS nuestras: el contenedor que aloja la capa de composición se reducía a cero de ancho, y dentro de él el desplazamiento `right` de esa capa no resolvía a nada en absoluto. La ventana de candidatos iba detrás, porque el sistema operativo la coloca a partir del rectángulo de esa capa. La capa vuelve a dibujarse y adopta los colores del tema de la aplicación, y el elemento de entrada invisible sobre el que se apoya libera su geometría en cuanto termina la composición, de modo que hacer clic y arrastrar sobre esa zona llega al terminal y no a un elemento vacío que antes seguía tapándolo.

- **La barra de título nativa sigue el ajuste de tema claro u oscuro.** La aplicación conserva la barra de título del sistema, y Windows la pinta en claro mientras no se le diga otra cosa, así que una interfaz oscura llevaba encima una franja blanca. Ahora coincide con la aplicación, incluidas las ventanas que se abren después, como las de SSH y las de conexión remota. Elegir «seguir al sistema» devuelve el control al sistema operativo en lugar de fijar un valor.

- **Desaparece el cuadrado suelto del arranque en frío.** El plugin de instancia única crea una ventana de mensajes oculta y nunca le daba la transparencia que su propio estilo prometía, así que Windows a veces agrandaba esa ventana de tamaño cero hasta su mínimo y pintaba un cuadradito durante el arranque. Ahora es transparente de verdad; el comportamiento de instancia única no cambia.

### Rendimiento

- **El acceso remoto descarga mucho menos en la primera carga.** Los recursos estáticos se comprimen ahora bajo demanda y se sirven con validadores de caché, de modo que una segunda visita revalida en lugar de volver a descargar, y los paquetes de idioma y los renderizadores opcionales del terminal se cargan solo cuando algo los necesita, en vez de formar parte del primer envío. En conjunto, la transferencia inicial baja a alrededor de una quinta parte de lo que era.

### Correcciones

- **Una sesión hija arranca ahora igual que la sesión que la pidió.** Las hijas no heredaban ni el modo de permisos del padre ni sus argumentos de inicio, así que la hija de una sesión que omitía las confirmaciones aparecía pidiéndolas, y un modelo fijado en el padre se perdía. Ahora se heredan los dos, con los valores predeterminados globales del tipo de agente como respaldo: los mismos que aplica el menú de «nueva sesión de agente».

- **Reconectar una ventana remota ya no informa de un falso «error de autenticación».** Cuando la ventana se reconectaba, el nuevo WebSocket y el que sustituía competían entre sí; el cierre del perdedor se informaba como un fallo de autenticación, y el aviso acusaba de rechazo a un emparejamiento perfectamente válido.

- **Un grupo ya no puede arrastrarse dentro de su propio subárbol.** Soltar un grupo sobre uno de sus descendientes desprendía toda esa rama del árbol, y las sesiones que contenía desaparecían de la barra lateral hasta que se reparaba la base de datos a mano. El movimiento se rechaza ahora.

- **La salida de los agentes conserva el color cuando VelaTerm se inicia desde otra herramienta.** Un terminal hereda el entorno de aquello que lo haya iniciado, así que arrancarlo desde un IDE o desde un entorno de agentes que exporta `NO_COLOR`, `CI` o `FORCE_COLOR=0` hacía que todas las TUI de agente se vieran monocromas dentro de VelaTerm, aunque el terminal anuncia color completo. Esos valores heredados se descartan al iniciar una sesión; las mismas variables exportadas desde tu propio perfil de shell siguen aplicándose, porque ese perfil se ejecuta dentro de la sesión.

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
