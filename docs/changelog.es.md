## v0.3.0 — 2026-11-08

- 🗂️ Las sesiones se dividen ahora en tres tipos, Tarea, Proyecto y Chat, que se eligen en la parte superior de la página «Nueva sesión», y la barra lateral izquierda se organiza en las colecciones fijas Proyectos, Terminales, Tareas y Chats, donde Terminales muestra todos los terminales en ejecución y los agentes detectados en ellos.
- 🧭 Una nueva barra de actividad en el borde izquierdo permite cambiar entre Proyectos, Terminales, Tareas, Chats, Sesiones archivadas y Base de conocimientos, y reúne en la parte inferior Ajustes, Cuenta, Comentarios, Compartir y Centro de juegos.
- 📦 Las colecciones pueden archivarse por completo con «Archivar colección» en su menú contextual, y al restaurar cualquiera de sus sesiones la colección vuelve a aparecer.
- 🆕 En la página «Nueva sesión», el proyecto, el agente, el modelo, el esfuerzo de razonamiento y los permisos se eligen en una tarjeta de ajustes situada sobre el cuadro de entrada, y la lista de proyectos permite crear, importar o clonar un proyecto sin salir de la página.
- 🩺 Al abrir una conversación se comprueba si su agente está instalado y tiene la sesión iniciada: si no es así, aparece un aviso de instalación o un panel de inicio de sesión, y «Enviar» permanece desactivado hasta que el agente esté listo.
- 🔧 Un agente solo se considera instalado si realmente se ejecuta, de modo que una instalación a medias o una actualización defectuosa muestra la guía de instalación, las instalaciones con un clic solo terminan cuando el agente se inicia, y una actualización tras la cual el agente no puede iniciarse se indica como incompleta.
- 🪟 El nuevo comando `vartifact <file|URL>` abre un archivo en un panel junto a la conversación, con vista previa en directo para HTML y vistas editables para Markdown, imágenes y código que se guardan con ⌘S, mientras que `vopen` ahora siempre abre una pestaña.
- 💤 Las sugerencias atenuadas en línea del terminal ahora están desactivadas de forma predeterminada y el autocompletado vuelve a «Solo Tab»; esta actualización cambia una sola vez el ajuste «En línea + Tab» existente a «Solo Tab», y puede volver a activarlas en los ajustes.
- ⌨️ En el modo «En línea + Tab», el nuevo ajuste «Tecla Tab» permite aceptar con Tab la sugerencia atenuada; un segundo Tab la deshace y muestra las coincidencias, y la barra de título del terminal indica qué teclas están disponibles mientras hay una sugerencia visible.
- 🎨 Editar una línea de comandos de Zsh ya no pone en gris el texto escrito previamente, y la descripción del modo de sugerencias en Ajustes coincide con el modo seleccionado.
- 📡 El autocompletado con Tab en conexiones remotas responde en un solo viaje de ida y vuelta.
- 💬 Las sesiones de ZCode permiten añadir indicaciones con Option/Alt+Enter, de modo que el mensaje se incorpora al turno en curso tras la siguiente llamada a una herramienta.
- 🔑 ZCode 3.15 y versiones posteriores pueden iniciar sesión con OAuth, y un GLM Coding Plan adquirido ya no aparece como inexistente o agotado.
- 📥 Se pueden importar sesiones de Pi, OMP, ZCode y DeepSeek Harness, y el cuadro de diálogo «Importar sesiones» permite filtrar por agente.
- 🔤 La interfaz usa Inter en todas partes, mientras que el código, la salida de las herramientas, las rutas y los terminales siguen en JetBrains Mono, y se ha eliminado el ajuste de fuente de la interfaz.
- 📚 La base de conocimientos muestra las entradas en una columna centrada, presenta las sesiones archivadas como un árbol de proyectos, grupos y sesiones principales con una vista «Por fecha», y permite alternar la base de conocimientos local entre tarjetas y lista.
- 🔔 En macOS y Windows, al hacer clic en una notificación se trae al frente su propia ventana, de modo que una notificación de una ventana remota ya no coloca la ventana principal por encima de ella.
- 📋 En macOS, pegar ya no bloquea la aplicación cuando la aplicación de origen del portapapeles deja de responder: VelaTerm desiste al cabo de 5 segundos y te lo indica.
- 📝 En el editor de código fuente, «Seleccionar todo» desde el menú de macOS o el menú contextual selecciona el archivo completo, y al copiar ya no se pierden las líneas fuera de la pantalla.
- 📏 El cuadro de entrada ya no crece y vuelve a encogerse después de enviar un mensaje.
- 📂 Las colecciones Terminales, Tareas y Chats muestran su contenido desplegado, independientemente de lo que esté contraído en Proyectos.
- 🔄 Las sesiones nuevas ya no fallan al sincronizarse con `cursor_expired` después de abrir la conversación de una subtarea.
- 🧯 Reconectarse tras una desconexión prolongada con mucho contenido acumulado ya no deja la conversación bloqueada con «Enviar» desactivado.
- 🪞 Las conexiones remotas a equipos Windows con Cygwin o Git Bash usan la aplicación de escritorio de VelaTerm que ya se ejecuta allí, por lo que enviar un mensaje ya no falla mientras esa aplicación ejecuta la sesión.
- ⚡ Indexar conversaciones largas de Codex consume aproximadamente la mitad de tiempo de CPU.
- 🔗 Los enlaces a artefactos de Claude se abren en tu navegador predeterminado.
- 🗄️ El botón «Sesiones archivadas» ha pasado de la barra de pestañas a la barra de actividad.

## v0.2.15 — 2026-11-03

- 🆕 Una pestaña «Nueva sesión» te permite elegir el proyecto, el agente, el modelo, el esfuerzo de razonamiento, los permisos, la vista y un worktree independiente alrededor de un cuadro de entrada centrado, y después inicia la sesión con tu primera tarea; ábrela desde «Nueva sesión…» en los menús de nueva sesión, con el botón de estrella de la barra de pestañas o desde el panel central vacío.
- 🐋 DeepSeek Harness (`dsh`) se incorpora como agente de la vista de conversación, con tarjetas de herramientas, razonamiento, solicitudes de permiso, cambio de modelo y de esfuerzo, uso del contexto e historial que se reanuda al volver a abrir la sesión.
- 🧩 Copilot, Cursor, Kimi y Grok ya pueden ejecutarse en la vista de conversación.
- 🤖 Zhipu ZCode se incorpora como agente de la vista de conversación: usa el agente integrado de la aplicación de escritorio de ZCode cuando está instalada, y puedes iniciar sesión con Z.ai o Zhipu BigModel, o cambiar entre ambos, sin salir de la conversación.
- 📊 Las sesiones de ZCode muestran en el panel Info los tokens de este turno, la tasa de aciertos de caché y la velocidad de generación, el uso del contexto y el uso de GLM Coding Plan en los periodos de 5 horas y semanal.
- 📦 ZCode (en macOS y Windows) y DeepSeek Harness se pueden instalar desde la conversación con «Instalar ahora», y el mensaje que falló se vuelve a enviar en cuanto termina la instalación.
- ⬆️ Al hacer clic en «Actualización» en la insignia de versión de un agente se abre un cuadro de diálogo que ejecuta el propio comando de actualización del agente con la salida en directo y ofrece «Reiniciar agente» para que la conversación continúe con la nueva versión.
- 🔌 Las conexiones remotas con muchos terminales activos ya no se desconectan una y otra vez después de reconectarse.
- 🔄 Tras reiniciarse un servidor remoto, las sesiones ya no se quedan bloqueadas sincronizándose mientras se inician sus terminales.
- 🔁 Cuando una conversación no logra sincronizarse, se vuelve a intentar automáticamente, y la zona situada bajo el cuadro de entrada muestra el estado con un botón «Reintentar».
- 🐢 Las conexiones SSH a un host remoto muy cargado ya no agotan el tiempo de espera mientras aguardan la respuesta del servidor.
- 🔗 «Reconectar ahora» en una ventana remota reconstruye el túnel SSH de inmediato en lugar de esperar al siguiente reintento automático.
- 🪟 En macOS y Windows, cerrar la ventana principal pide confirmación y deja abiertas las ventanas remotas; en macOS la aplicación permanece en el Dock, y al volver a abrir la ventana principal se restaura tu espacio de trabajo guardado.
- 🧾 Los archivos modificados de la lista «This turn» y del panel Git abren sus diffs en una pestaña en lugar de en un cuadro de diálogo.
- ⌨️ Las sugerencias de comandos del terminal ya no aparecen mientras escribes: Zsh muestra en gris, en línea, el comando más reciente del historial que coincide, que se acepta con → o Fin, y la lista solo aparece al pulsar Tab; elige «En línea + Tab», «Solo Tab» o «Desactivadas» en Ajustes.
- 💬 En las vistas previas de Artifact puedes hacer clic en elementos de la página para dejar comentarios a la sesión, y las opciones marcadas en la página aparecen sobre la vista previa para que puedas saltar de una a otra y elegir una.
- 🔍 La búsqueda dentro de una conversación es de texto completo, encuentra todas tus palabras en cualquier orden y muestra «Sin resultados» en lugar de «Cargando la conversación…» cuando no hay coincidencias.
- 🧰 Las habilidades incluidas, como vspawn y vrefer, ahora se instalan en la carpeta compartida `~/.agents/skills`, de modo que Pi, ZCode y DeepSeek Harness pueden usarlas, y Codex ya no muestra cada habilidad dos veces.
- 📋 Los resultados de `/code-review` en la vista de conversación aparecen como una lista de problemas encontrados con ubicaciones de archivo en las que se puede hacer clic.
- 🌐 Cuando la traducción de respuestas está desactivada en una sesión, las tarjetas de preguntas ofrecen un botón «Traducción» que traduce solo esa tarjeta.
- ✂️ Las preguntas y las opciones de las tarjetas de preguntas se pueden seleccionar y copiar.
- 🌳 El árbol de sesiones coloca bajo la fila de cada worktree los grupos y las sesiones principales que pertenecen a él, incluidas las sesiones iniciadas directamente en la carpeta del worktree.
- 🌲 Los grupos anidados usan el worktree del grupo superior más cercano que tenga uno para las sesiones nuevas, las acciones de Git y las fusiones.
- 🔎 Renombrar un elemento mientras el árbol de sesiones está filtrado ya no hace que desaparezca de los resultados.
- ⏳ Cuando OpenCode alcanza un límite de uso, una sola línea de error indica de qué límite se trata, y la conversación continúa automáticamente tras el reinicio del límite cuando OpenCode informa de la hora de reinicio.
- 📸 La superposición de captura de pantalla aparece más rápido después de pulsar el atajo.
- 🚀 Las ventanas muestran un esquema de la disposición en cuanto se abren y cargan menos de la mitad de código al iniciarse.
- ⚡ `vspawn --yes` crea la sesión hija directamente, sin mostrar la tarjeta de inicio.
- 🛡️ Las auditorías de código usan Codex Security 0.8.2 y pueden ejecutarse con DeepSeek, mientras que los formularios de auditoría y de «Añadir a la base de conocimientos» usan los mismos campos de agente, modelo y esfuerzo que las sesiones nuevas y admiten modelos que no figuran en la lista.
- 🌗 En macOS, cuando el tema sigue al del sistema, la barra de título se oscurece junto con el resto de la ventana.
- 📥 Las instalaciones con un clic solo indican que han terminado cuando el comando de instalación ha finalizado, y una instalación incompleta de Zoo Code se considera no instalada en lugar de provocar un fallo al iniciar.
- 🖥️ Cambiar una sesión a la vista de terminal ya no falla con «The initial task was already claimed».
- 🧟 Las sesiones ya no indican «The previous agent is still running» cuando otro programa ha reutilizado el ID de proceso del agente anterior.
- 🧯 Después de que el navegador se reconecte, las conversaciones ya no conservan una línea TransportError de la desconexión.
- 🪞 Con la duplicación activada, abrir y cerrar pestañas ya no convierte las sesiones de conversación en marcadores de posición «Restaurado desde el espacio de trabajo guardado».
- 📜 Las conversaciones de Claude que contienen una respuesta descartada vuelven a cargar su historial con normalidad.
- 🧠 Los modelos integrados de Claude que faltan en el catálogo de modelos del sitio web siguen apareciendo en los formularios de nueva sesión y de inicio.

## v0.2.14 — 2026-10-23

- 🔌 Las conexiones remotas ya no se cortan y se reconectan cada segundo cuando hay varias conversaciones largas abiertas, y esas conversaciones vuelven a abrirse con normalidad.
- ⏹️ Las conexiones SSH y WSL muestran un botón «Detener» mientras se conectan, para detenerlas en cualquier fase, incluso mientras se prepara el servidor; durante una conexión, «Cancelar» en el cuadro de diálogo de contraseña también la detiene.
- 🌿 El árbol de sesiones y las etiquetas de rama ya no acumulan procesos de Git en repositorios grandes, y las lecturas de Git en segundo plano se detienen a los 30 segundos en lugar de quedarse bloqueadas.
- 🧩 El panel Git se actualiza mucho más rápido en repositorios con submódulos: los cambios dentro de un submódulo ya no hacen que aparezca en el repositorio principal, mientras que un submódulo que apunta a un commit nuevo sigue apareciendo.
- 📁 Los grupos se pueden arrastrar a otros proyectos junto con sus grupos anidados y sus sesiones, y a partir de entonces esas sesiones se inician en la carpeta del nuevo proyecto.
- ⏳ Al abrir una conversación larga aparece un pequeño indicador «Cargando la conversación…» en lugar de «Se está indexando el historial».
- 🖱️ El menú contextual de una sesión muestra «Terminar proceso» mientras el proceso de la sesión está en ejecución, incluso si se inició desde otro cliente, y lo oculta cuando el proceso termina.

## v0.2.13 — 2026-10-22

- 📱 En el teléfono, los agentes que se ejecutan en la vista de terminal se abren directamente en el terminal, con un cuadro de entrada flotante para escribir o dictar mensajes completos; los pequeños botones «Transcripción» y «Terminal» del encabezado alternan entre ambos, y la transcripción ya no sugiere cambiar a la vista de conversación.
- 🔑 En las conexiones compartidas, el propietario del equipo puede abrir el terminal de los agentes que se ejecutan en la vista de terminal y escribir en él, y la lista de sesiones también muestra agentes sin vista de conversación, como Kiro; las cuentas invitadas siguen viendo una transcripción de solo lectura.
- 🌐 Traducir manualmente una respuesta en una sesión inactiva muestra la traducción en lugar de «La traducción no está disponible».
- 📜 Mientras te desplazas hacia arriba en una conversación larga, las respuestas nuevas ya no quedan detrás de «Cargar mensajes más recientes», y al bajar desde el historial anterior los mensajes más recientes se cargan automáticamente.
- 🧭 La vista para teléfono ya no ofrece «Cambiar a versión de escritorio», y los teléfonos que cambiaron antes vuelven automáticamente a la vista para teléfono.
- 🧹 Los registros de ejecución de `vrun` se eliminan automáticamente 24 horas después de que termine la tarea.

## v0.2.12 — 2026-10-20

- 🛑 Detener un turno de Claude en la vista de conversación surte efecto de inmediato y el siguiente mensaje se envía al momento, incluso cuando el equipo está muy cargado.
- 🪟 Los atajos de menú como Cmd+D y los clics en notificaciones solo actúan en la ventana correspondiente, así que las ventanas remotas ya no se dividen junto con la ventana principal.
- 🔎 La búsqueda global devuelve resultados mientras la indexación sigue en curso e indica cuántas sesiones faltan por indexar.
- ⚡ La búsqueda global indexa las conversaciones de Claude, Codex, Pi y OMP directamente desde sus archivos, con mucha más rapidez y sin llenar la caché del historial de conversaciones, y al abrir un resultado se salta al mensaje exacto.
- 🧠 El historial de conversaciones se lee directamente de los registros originales de los agentes en lugar de una copia aparte en el disco, y las carpetas `chat-index` y `chat-spool` se eliminan al iniciar para liberar espacio en disco.
- 📱 En el teléfono, las sesiones que se ejecutan en la vista de terminal sugieren cambiar a la vista de conversación; al confirmarlo, la aplicación de escritorio también cambia la sesión.
- 👀 Las conexiones compartidas muestran en modo de solo lectura las sesiones que se ejecutan en la vista de terminal, y solo el propietario del equipo puede cambiarlas a la vista de conversación.
- 🌐 La configuración de traducción añade «Activar la traducción de forma predeterminada», que fija el estado inicial de la traducción en las sesiones sin una elección guardada.
- 🌳 El árbol de sesiones agrupa bajo una sola fila de worktree las sesiones que usan el mismo worktree.
- 📄 Las respuestas breves en la vista de conversación, incluidos los mensajes con imágenes, ya no muestran el enlace «Leer el contenido completo».
- 🔕 El botón que borra todas las insignias de notificación pasa al extremo izquierdo de la barra inferior del árbol de sesiones.
- 🤖 Los diálogos de nueva sesión y de reanudar sesión muestran el icono de cada agente en la lista de tipos de agente, y la lista de permisos se abre hacia arriba cuando no hay espacio suficiente debajo.
- 🩹 La vista de conversación ya no indica que el historial contiene registros dañados cuando el almacenamiento está ocupado, y sigue cargando poco después.
- 📝 En el editor de Markdown, Retroceso y Mayús+Tab quitan la numeración de un elemento de lista vacío o de un párrafo sin numerar dentro de una lista.
- 🔵 El chip «Tareas» de la vista de conversación se vuelve cian, el color que usa el árbol de sesiones para el trabajo en segundo plano, mientras sigan en curso tareas en segundo plano, y en la lista de tareas las que están en curso también aparecen en cian con la etiqueta «En ejecución».
- 🗑️ Al eliminar una sesión ya no se ofrece quitar los worktrees de git; para quitar uno, usa «Delete worktree…».
- 🔁 El estado de la traducción se actualiza cuando cambia, en lugar de cada 10 segundos, y la configuración de traducción vuelve a comprobar los agentes instalados cada vez que se abre.
- 💤 El panel Info pausa el muestreo de recursos del proceso mientras la ventana está oculta.
- ⏱️ El indicador de trabajo y el temporizador del turno en la vista de conversación ya no desaparecen ni se quedan visibles por error cuando un turno empieza o termina mientras cambias de sesión.
- 🔗 `vrefer` numera los mensajes igual que `vsearch`, y en las sesiones de Codex bifurcadas, `vrefer` y la búsqueda global incluyen el historial de la conversación de origen.

## v0.2.11 — 2026-10-18

- 🧪 Las vistas previas HTML experimentales se abren junto a las conversaciones o en pestañas, se actualizan al cambiar los archivos y ofrecen controles de código, consola y versiones; la validación nativa y multiplataforma sigue pendiente.
- 📜 Las conversaciones largas cargan los mensajes anteriores por páginas y despliegan el contenido extenso a petición, sin duplicar mensajes ni herramientas en las conversaciones en directo de Claude y Codex.
- 🔎 La búsqueda global actualiza más rápido las conversaciones modificadas, reanuda la indexación tras interrupciones y omite las conversaciones ilegibles mientras sus archivos no cambien.
- 📊 El recuento de archivos modificados del panel Info abre una lista de los archivos editados en este turno con las líneas añadidas y eliminadas cuando están disponibles.
- 🗂️ Las conversaciones archivadas siguen sus proyectos, grupos y sesiones superiores originales en filas compactas, con proyectos plegables y controles para expandir o contraer todo.
- 🧩 Las habilidades de los complementos de la cuenta de Codex aparecen en las sugerencias de autocompletado y los menús permiten seleccionar con las flechas.
- ⌨️ En Windows y Linux, Ctrl+Mayús+C copia el texto seleccionado en el terminal y puede cambiarse en Ajustes ▸ Atajos.
- 🧹 Tras ejecutar `/clear` en el terminal de Claude, volver a abrir la sesión o cambiar a la vista de conversación ya no reanuda la conversación borrada.
- 📏 El panel Info muestra el límite de contexto del modelo en las conversaciones de OpenCode.
- 🌐 La traducción incorpora Bing Translator sin clave API, ajustes de idioma compactos y traducción antes de reenviar mensajes editados de Claude y Codex.
- 🔤 Las traducciones existentes siguen visibles al cambiar el motor o modelo de traducción y pueden volver a traducirse.
- ⏱️ `vrun` informa del progreso cada 180 segundos con el código de salida 75, permite seguir esperando con `--wait` y protege las tareas activas frente a la reutilización de su nombre.
- 🧵 Las respuestas de Claude iniciadas al terminar tareas en segundo plano tienen su propio resumen de tarea y separador de conversación, también en el historial.
- ⚠️ Cambiar entre las vistas de terminal y conversación requiere confirmación si hay tareas en segundo plano en ejecución.
- 🏠 Las sesiones sin directorio de trabajo o cuyo directorio ya no existe se inician en la carpeta personal en ambas vistas.
- 🪟 En Windows, iniciar VelaTerm o abrir el panel de conexión ya no solicita instalar WSL si no hay distribuciones registradas.
- 🔐 Los inicios de sesión remotos fallidos cuentan para el límite de intentos aunque la conexión se cierre durante la verificación de la contraseña.
- 🏷️ Las insignias de versión del agente muestran un enlace explícito `Update: vX` cuando hay una versión más reciente del CLI.
- 🔵 Los indicadores de tareas en segundo plano permanecen en cian fijo y los puntos de estado se mantienen en las pestañas y la barra lateral, sin aparecer en los títulos de los paneles.
- ✍️ Los bordes de la entrada de conversación son más claros, con un contorno exterior al recibir el foco, y los valores del panel Info tienen el mismo brillo que sus etiquetas en el tema oscuro.
- 🌿 Los botones para clonar proyectos usan un icono de rama con un marcador de proyecto.

## v0.2.10 — 2026-10-13

- 🧩 Se admite OpenCode v2 además de v1, y la lista de modelos se carga correctamente con OpenCode v2.0.26.
- 🏷️ La cabecera de la sesión muestra la versión de la CLI del agente; si hay una versión más reciente, el número de versión enlaza con las instrucciones oficiales de actualización.
- 🗂️ El panel Git muestra hasta 1000 archivos modificados y propone añadir carpetas grandes como `node_modules` a `.gitignore`.
- 🔤 El panel Git muestra correctamente los nombres de archivo con caracteres no ASCII, prepara, quita del área de preparación y descarta solo los archivos que eliges, y agrupa los conflictos por separado.
- 📜 El historial de Git conserva el rango cargado, los elementos desplegados y la posición de desplazamiento al actualizarse.
- 🌿 La rama que se muestra en el panel derecho, la barra de estado, el título del terminal y la barra lateral se actualiza en cuanto cambias de rama, sin reiniciar.
- 📁 El árbol de archivos actualiza automáticamente las carpetas abiertas y marca las que contienen cambios de Git.
- 🧹 Windows: las actualizaciones automáticas ya no dejan instaladores en la carpeta Temp y se eliminan los restos de actualizaciones anteriores.
- 🪝 Windows: Codex, Crush, Kiro, Cursor, Kimi, Antigravity, Copilot y Grok informan del estado de la sesión directamente a VelaTerm, sin scripts de PowerShell.
- 🖱️ Windows: los programas de terminal a pantalla completa, como vim, reciben los clics y el desplazamiento del ratón; la rueda ya no se convierte en teclas de flecha.
- 🕘 Los terminales zsh vuelven a leer y escribir tu propio `~/.zsh_history`, así que `history` y la flecha arriba también muestran los comandos de otros terminales.
- 🖥️ Las nuevas sesiones de Claude y Codex se abren en la vista de conversación; los demás agentes se abren en el terminal de forma predeterminada.
- 🚀 `vspawn` admite `--pi` y `--agent <nombre>`, por lo que puede iniciar cualquier agente que acepte tareas.
- ⏱️ Si la lista de modelos no se carga en 30 segundos, se muestra un error con Retry.
- 🗑️ Los preajustes guardados se pueden eliminar en Ajustes ▸ Agentes.
- ⌨️ La información emergente del botón de captura de pantalla muestra el atajo actual, y su icono se distingue mejor del botón de compartir.
- 🧭 El panel derecho vuelve a cambiar de Conocimiento a Info y a las demás pestañas.
- ✍️ macOS: escribir `/` en el terminal con un método de entrada ya no lo introduce dos veces.

## v0.2.9 — 2026-10-07

- ✨ El cambio de nombre con IA recuerda el modelo y el nivel de razonamiento elegidos para cada agente.
- ⛔ La generación de títulos con IA se puede cancelar con Cancel, Esc o un clic fuera del diálogo, y después se puede volver a intentar.
- 🧭 Los nuevos flujos Plan/Execute tienen el rol Review independiente desactivado por defecto y permiten activarlo en el diálogo de inicio o con `--review`.
- 🤖 El selector de modelos de Chat muestra los estados de carga, error y lista vacía con Retry, y las opciones de Codex corresponden a la cuenta y la configuración de la sesión actual.
- 📊 El panel Info muestra una estimación de la velocidad media de salida, incluidos los tokens de razonamiento registrados, o un guion cuando no se puede calcular.
- 🩺 Los ajustes avanzados ofrecen un diagnóstico opcional de la latencia de entrada con un umbral ajustable, sin registrar el texto que escribe.
- ✂️ La herramienta experimental de capturas de pantalla tiene un botón en la barra de título de las plataformas de escritorio Tauri compatibles, incluso cuando su atajo de teclado está desactivado.
- 🌬️ Los indicadores cian de tareas en segundo plano usan la misma animación de pulsación en el árbol de sesiones, los filtros de estado y la barra de estado.
- 📁 Los grupos creados con un filtro de estado activo permanecen visibles hasta actualizar el estado o cambiar el filtro, siempre que también coincidan con la búsqueda por nombre.
- 🎨 Los registros de comandos en segundo plano muestran colores, estilos de texto y la última línea de progreso, y la barra de ejecución deja visible el indicador de foco del panel.

## v0.2.8 — 2026-10-06

- 🪟 Windows: al volver a la ventana, el foco del teclado se mantiene donde está si ya se encuentra dentro de la página.

- 🛡️ Windows: el antivirus Huorong ya no detecta VelaTerm ni vela-server como `Trojan/MSIL.ShellLoader.q`. Se trataba de un falso positivo: para ejecutar comandos de shell desde la vista de conversación, VelaTerm creaba cada proceso en estado suspendido y después lo reanudaba, una secuencia de llamadas que también utiliza el software malicioso para inyectar código. Ahora los procesos se inician de forma normal y se ha eliminado el código que provocaba la detección.

- 🔕 Windows: al crear sesiones y actualizar la lista de modelos ya no se ejecuta PowerShell en segundo plano, y el estado de las sesiones de Cursor ahora se determina a partir de la salida del terminal, lo que puede ser algo menos preciso.

- ⛔ Windows: en la vista de conversación, las entradas que empiezan por `!` ya no se ejecutan como comandos de shell; aparece un aviso y la entrada se conserva, mientras que los comandos anteriores del historial siguen mostrándose.

- 🔐 Las conexiones SSH a hosts Windows con el servidor OpenSSH integrado ya no inician PowerShell con `-EncodedCommand`.

- 🚀 Las sesiones de agentes TUI se inician correctamente aunque el PATH sea largo, sin mostrar un comando de inicio truncado en el terminal.

- ⬇️ Las sesiones de agentes TUI vuelven al final cuando termina un turno o el agente solicita una aprobación, salvo que esté consultando la salida anterior.

- 💾 Se ha retirado la recuperación experimental de conversaciones de la v0.2.7: Chat ya no muestra mensajes Saved submission duplicados ni avisos de interrupción, y los mensajes en cola no enviados ya no se conservan tras un reinicio.

- 🔁 Los flujos de trabajo de planificación y ejecución incorporan un rol Review independiente y opcional, activado de forma predeterminada en los nuevos flujos, con su propio agente, modelo y nivel de razonamiento; `vspawn --plan-execute` admite `--review`, `--no-review`, `--review-agent`, `--review-model` y `--review-effort`.

- 🏷️ «Renombrar con IA…» pide confirmación antes de empezar y permite elegir el agente, el modelo y el nivel de razonamiento para ese cambio de nombre; el cuadro de diálogo aparece centrado mientras se cargan las opciones.

- 🔍 Las sesiones, los grupos y los proyectos se pueden arrastrar mientras hay activa una búsqueda por nombre, un filtro de estado o un filtro de marca.

- ➕ Con un filtro de estado activo, una sesión nueva permanece bajo su grupo o sesión principal hasta que se actualice el estado o se cambie el filtro.

- 🪟 Las ventanas de la aplicación se abren más grandes de forma predeterminada, según el área disponible de la pantalla, y las ventanas de conexión se abren centradas en la pantalla de la ventana principal.

- 🎨 Las colecciones tienen un nuevo icono de capas apiladas, los iconos de la barra de título oscura son algo más tenues, los cuatro botones de la barra lateral vacía tienen el mismo ancho y los cuadros de diálogo de formulario usan un único estilo de etiqueta.

---

## v0.2.7 — 2026-10-05

- 🪐 Antigravity se incorpora a la vista de conversación como agente experimental con texto, herramientas, mensajes en cola e historial nativo; las imágenes, las instrucciones durante una respuesta, las aprobaciones interactivas, la bifurcación y el retroceso siguen sin estar disponibles, y queda pendiente la validación completa de la recuperación en todas las plataformas.

- 🗂️ Las colecciones admiten subcolecciones, proyectos y grupos de sesiones, se pueden crear o importar desde el espacio de trabajo y conservan sus proyectos y sesiones archivadas al eliminarlas.

- ⚠️ Las versiones anteriores no deben usar una base de datos cuyas carpetas se hayan migrado a colecciones; para volver a una versión anterior hay que restaurar una copia de seguridad previa a la migración.

- 🏷️ El cambio de nombre inteligente asigna un nombre a la sesión según su conversación con la configuración del agente actual y permite elegir otro agente cuando esa configuración no se puede usar.

- 🌱 Las nuevas sesiones utilizan el entorno shell actualizado y los agentes recién instalados sin reiniciar VelaTerm.

- 💾 Tras un reinicio, la recuperación experimental de conversaciones restaura los mensajes, las imágenes y las entradas en cola guardados y pausa el trabajo interrumpido hasta que se reanude explícitamente; queda pendiente la validación completa en entornos nativos y en todas las plataformas.

- 📸 La captura de pantalla experimental de las aplicaciones Tauri para macOS y Windows permite seleccionar una zona, añadir anotaciones, copiarla o guardar el PNG original; queda pendiente la validación de las aplicaciones nativas y no se admite en Electron, el navegador ni las vistas remotas.

- 🧵 Las pestañas de tareas de Claude en segundo plano muestran todo su historial nativo de conversación, incluidas las llamadas a herramientas en paralelo.

- 🖼️ Cada mensaje de conversación admite hasta 20 imágenes, con un límite de 5 MiB por imagen.

- 📍 Los marcadores de mensajes permiten previsualizar el contenido y saltar directamente a mensajes anteriores del usuario.

- 🧭 Los selectores de directorios para proyectos, clonar y Guardar como ofrecen de forma uniforme la edición de rutas y el autocompletado de ubicaciones.

- 🪪 `vself` lee la configuración guardada de las sesiones y sus relaciones entre padres e hijos, mientras que `vflow list` enumera los flujos de planificación y ejecución relacionados sin modificarlos.

- ⌨️ Seleccionar todo en el terminal se configura en los atajos, usa por defecto Cmd+A en macOS y Ctrl+Shift+A en otras plataformas y solo se aplica cuando el terminal tiene el foco.

- 🐚 El autocompletado de Bash admite rutas que empiezan por `~`, y las versiones antiguas de Bash abren sesiones sin errores de PS0.

- 🛑 Los hooks de tareas secundarias ya no cambian el estado de conversación de su sesión principal, y cerrar una pestaña de conversación detiene su proceso Chat activo.

- 🪟 Windows recupera el foco del teclado del terminal al volver a la ventana y elimina la línea azul durante la composición de texto chino.

- 🎨 El tema oscuro clásico ofrece un contraste más claro, los iconos del escritorio siguen el tema del sistema, los de proyectos son azules y las colecciones ya no muestran el número de proyectos.

- 📚 Los menús de sesiones y colecciones usan una etiqueta más clara para añadir contenido a la base de conocimientos.

- 🌐 El acceso a VelaTerm mediante HTTP sin cifrar en una red local abre la página correctamente.

- 🌍 Las comprobaciones de actualizaciones incluyen el idioma de la interfaz para mostrar las notas de versión correspondientes, y el servicio admite el actualizador Electron de Linux.

- 🧰 Los nombres de agentes Kimi Code y Grok Build ya no incluyen el número de versión del modelo.

---

## v0.2.6 — 2026-09-30

- 🐧 La versión para Linux ahora se basa en Electron. El AppImage conserva su nombre de archivo y tus datos, ya no requiere WebKitGTK ni libfuse2, y las instalaciones existentes se actualizan a ella mediante el actualizador integrado.

- 🛰️ Los equipos a los que no se puede acceder por SSH, como WSL en otro ordenador o un contenedor Docker, pueden vincularse a tu cuenta tras instalar `vela-server` con un solo comando y aparecen entonces en la lista Remote para conversar con la IA. Cuando el propietario concede acceso completo en el host, la aplicación de escritorio también puede usar su terminal, sus archivos y su panel Git.

- 📂 Las carpetas que se arrastran a la barra lateral desde Finder, el Explorador de archivos o un gestor de archivos de Linux se añaden como proyectos.

- 📝 El editor de Markdown incorpora los atajos de Typora para encabezados, listas, bloques de código y tablas, el cierre automático de paréntesis y comillas, la apertura de enlaces con Cmd/Ctrl+clic, los modos de concentración y máquina de escribir, y el recuento de palabras y caracteres en la barra de estado. El front matter YAML aparece en un cuadro independiente sobre el documento y no cambia a menos que lo edites.

- 🗃️ Cuando una carpeta no es un repositorio Git pero contiene varios, el panel Git ofrece un selector de repositorio y trabaja con el repositorio elegido.

- ⏳ En la vista de conversación, una sesión cuya respuesta ha terminado mientras siguen ejecutándose tareas en segundo plano muestra el nuevo estado «Tareas activas», con un punto cian, en lugar de quedarse en «En curso». Las sesiones que ejecutan tareas largas con `vrun` muestran el mismo estado.

- 🍴 En la vista de conversación, una sesión bifurcada inicia su propia conversación la primera vez que se ejecuta, y la sesión de origen ya no recibe sus mensajes.

- 🛡️ En la vista de conversación, la etiqueta de modo de una sesión de Claude sigue el modo de permisos que Claude usa realmente, por ejemplo después de entrar en el modo Plan, y las tarjetas de permisos muestran el motivo que Claude indica para su solicitud.

- 📏 El cuadro de entrada de la vista de conversación se puede redimensionar arrastrando su borde superior, y un doble clic en el borde restablece la altura predeterminada. La altura se aplica a todos los paneles y se conserva tras reiniciar.

- 🗂️ Cuando se seleccionan varias carpetas en la barra lateral, el menú contextual permite archivarlas junto con sus sesiones.

- 📚 Las colecciones aparecen siempre por encima de los proyectos en la barra lateral.

- 🧠 Cuando un registro de procesamiento de la base de conocimientos ha creado una sola entrada, su título abre esa entrada directamente, y los botones de entrada muestran los títulos en lugar de números.

- 📊 El panel Info muestra el límite de contexto correcto para los modelos de Claude más recientes, como Opus 5.5, en lugar de 200k.

- 🎨 Cambios menores: las tres opciones de apertura en panel dividido del menú contextual de las sesiones se agrupan en el submenú «Abrir en panel dividido», la confirmación de salida avisa de que las ventanas remotas abiertas también se cerrarán y las listas desplegables de los formularios se cierran en cuanto se elige una opción.

---

## v0.2.5 — 2026-09-28

- 🐧 La aplicación de escritorio para Windows permite abrir espacios de trabajo independientes en WSL1 y WSL2 con los agentes, archivos e historial de sesiones de la distribución de Linux seleccionada.

- 🔄 Los espacios de trabajo de WSL permiten volver a conectarse cuando el servidor se detiene, y al cerrar una ventana puede detener el servidor o mantener las sesiones en ejecución.

- 🌐 Las sesiones de Claude incluyen la opción «Chrome» en el área de entrada, que activa o desactiva Claude in Chrome sin reiniciar la conversación. Cada sesión guarda su propia elección y las sesiones sin elección usan el valor predeterminado.

- 📟 La pestaña de una tarea de shell en segundo plano muestra el comando que ejecuta y su salida más reciente, que se actualiza mientras la tarea está en ejecución.

- 🧩 Si las Vela Skills no están instaladas, la barra de estado ofrece instalarlas. El cuadro de diálogo describe cada una, permite instalarlas directamente y puede configurarse para no volver a recordarlo.

- 🪪 Las sesiones hijas creadas con `vspawn` reciben el ID de su sesión padre en `VLX_PARENT_SESSION_ID`, de modo que un agente puede comunicarse con ella mediante `vrefer` y `vtell`. El nuevo comando `vself` muestra la sesión actual y las sesiones superiores.

- 🖥️ El AppImage de Linux ya no abre una ventana en blanco en distribuciones recientes como Fedora 44.

- 🎨 Cambios menores: «Tareas en segundo plano» aparece de forma predeterminada debajo del área de entrada y permanece en el menú «Más» si se vuelve a mover allí; cuando un enlace no se puede abrir, un mensaje indica que puede copiar su dirección con el botón derecho.

---

## v0.2.4 — 2026-09-26

- 📋 Al copiar desde la conversación se obtiene el texto tal como aparece en pantalla: sin comillas invertidas alrededor del código en línea, sin asteriscos en el énfasis, los enlaces reducidos a su texto, los bloques de código sin delimitadores, las celdas de tabla separadas por tabuladores y las listas con las viñetas que se ven. El formato enriquecido se sigue colocando también en el portapapeles, y el menú contextual incorpora «Copiar como Markdown» para obtener el código Markdown de la selección.

- ⏳ La continuación automática tras el reinicio de un límite de uso viene activada de forma predeterminada. Si usted ya había cambiado ese ajuste, se respeta su elección.

- ↩️ Revertir una conversación ya no queda bloqueado por una tarea que ha terminado: una tarea en primer plano se considera finalizada al acabar su turno, y el progreso que llegue después no vuelve a marcarla como en ejecución. Cuando realmente queda una tarea en segundo plano en marcha, el aviso indica su nombre.

- 🧹 Los hilos de subagentes de Codex ya no aparecen en la lista del historial de sesiones, igual que ocurre con las cadenas laterales de Claude.

- ⌨️ Autocompletado en el terminal: después de recorrer los candidatos con las flechas, Intro acepta el resaltado igual que el tabulador. Intro se sigue enviando al shell si no ha movido la selección, si siguió escribiendo tras elegirla o si mantiene pulsada una tecla modificadora.

- 📱 Android se compila en dos canales. La versión predeterminada incluye los canales de notificaciones de Getui, Huawei, Xiaomi, OPPO, vivo, Meizu y Honor; la versión de Play prescinde de ellos e indica que las notificaciones de tareas no tienen canal configurado.

- 🎨 Cambios menores de interfaz: las listas de tareas ya no repiten la casilla, la barra de notificaciones y el aviso de continuación automática se sitúan entre la conversación y el campo de escritura con el ancho del texto de los mensajes, y en el teléfono el botón de enviar permanece junto al campo cuando las opciones están plegadas.

---

## v0.2.3 — 2026-09-24

- 🪟 Las sesiones existentes se pueden llevar a los paneles divididos. El menú contextual de la barra lateral abre una sesión en un panel a la derecha, un panel abajo o el panel activo; al arrastrar una sesión desde la barra lateral hasta el borde de un panel, la división sigue esa dirección, y al soltarla en el centro se sustituye la sesión que se mostraba allí. Entre dos y cuatro sesiones seleccionadas se pueden disponer en mosaico dentro de una misma pestaña dividida a partes iguales, y las sesiones presentes en los demás paneles de la pestaña actual quedan señaladas en la barra lateral.

- 🗂️ Las tareas en segundo plano que inicia un agente se abren en sus propias pestañas junto a la conversación, con el estado, el tiempo transcurrido, los tokens, las llamadas a herramientas, la última herramienta informada y las fases de cada agente. Cada tarea tiene su propia dirección y, al salir de ella, se vuelve al panel desde el que se abrió.

- 💬 En la vista de conversación, un mensaje que empieza por `!` se ejecuta en el shell de la sesión. La salida aparece a medida que llega, se muestra el código de salida, el comando se puede cancelar mientras se ejecuta y queda en el historial de lectura de las sesiones de Claude, Codex, OpenCode, Pi y OMP.

- ⏱️ El nuevo comando `vrun` inicia un comando de larga duración y lo espera en una sola llamada, de modo que el agente sabe cuándo ha terminado realmente el trabajo. Los comandos iniciados así se muestran encima del terminal con su tiempo de ejecución, una ventana de registro y un botón de detención que pide confirmación.

- ⌨️ La creación de sesiones de agente tiene su propia página y su atajo de teclado: busque agentes y preajustes, reutilice el último empleado y decida si la sesión se crea al mismo nivel que la actual o por debajo de ella.

- 🧰 La barra de herramientas de escritura se configura en los ajustes: elija qué elementos aparecen junto al mensaje y en qué orden. Los elementos desactivados, igual que los que no caben en el ancho disponible, siguen accesibles desde el menú Más.

- 📥 Al descargar un archivo en una ventana de conexión por URL o SSH, primero se elige dónde guardarlo en este equipo y después se muestra el progreso de la descarga, con un botón para cancelarla.

- 🗃️ Las sesiones de Kiro se pueden importar a un proyecto y consultar: la importación empareja las sesiones por directorio de trabajo y cada una se abre en una vista de historial de solo lectura con búsqueda. Por ahora se admiten los registros de solo texto.

- 🧠 La organización de la base de conocimiento funciona con Grok, OpenCode, Pi y OMP además de Claude y Codex, y el agente se elige en el mismo tipo de lista desplegable que en el resto de la aplicación.

- 📱 iOS y Android: la página de inicio conserva sus conexiones SSH y URL, permite iniciar sesión en una cuenta de VelaTerm y enumera los dispositivos que comparten contenido en ella. La huella de un host se confirma una vez y queda memorizada, un código QR rellena la dirección del servicio, y todas las pantallas nativas y los avisos del sistema están traducidos a los 11 idiomas de la interfaz.

- 🔐 Las sesiones de Claude arrancan en el modo de permisos elegido: los argumentos de inicio adicionales conservan la prioridad prevista, el modo se contrasta con el que informa la CLI al arrancar y una sesión iniciada sin confirmaciones mantiene esa indicación.

- 🧩 El menú de modelos incluye Opus 5.5 y añade los modelos adicionales que informa la CLI seleccionada manteniendo el orden original; un modelo que llega con una actualización de la CLI aparece sin reiniciar la aplicación.

- 🌱 Las solicitudes de sesión derivada resisten las interrupciones: una solicitud cuya respuesta se perdió se puede recuperar tras volver a conectar, un inicio ya confirmado reutiliza la misma sesión y los mismos ajustes al reintentarlo, y un árbol de trabajo que no se pudo crear se revierte sin tocar nada de lo que ya existía.

- ⚡ La aplicación arranca más rápido: el código que se carga al inicio ocupa aproximadamente la mitad que antes, y las páginas de base de conocimiento, auditoría de seguridad, importación de sesiones y proyectos compartidos se cargan al abrirlas.

- 🖼️ Las imágenes pegadas o arrastradas a un mensaje se reducen a 1568 píxeles en su lado largo antes de enviarse.

- 🐚 Las sesiones de Bash cargan el autocompletado del shell desde un archivo de inicio, de modo que una sesión nueva ya no se abre con un comando escrito. Los archivos de perfil de inicio de sesión se siguen leyendo en el orden propio de Bash.

- ✍️ Markdown: una tilde suelta ya no tacha el resto de la línea, por lo que un indicador de shell pegado se mantiene legible, y la negrita o la cursiva que termina junto a caracteres chinos, japoneses o coreanos se cierra correctamente en lugar de dejar asteriscos a la vista.

- 📨 `vtell --steer` entrega el mensaje dentro del turno que el destinatario está ejecutando, sin esperar a que ese turno acabe. Si el destinatario está detenido en una pregunta, la respuesta es blocked, porque el mensaje solo se lee cuando esa pregunta se responde.

- 🔁 En la vista de conversación, el estado de una sesión termina en cuanto acaba su turno, la marca de no leído se borra cuando el trabajo empieza de verdad, y una sesión con trabajo en segundo plano en curso se mantiene marcada como activa.

- 🪟 Windows: los hooks de Cursor se inician correctamente y se ha actualizado la capa de ventanas por los problemas de entrada de teclado comunicados tras una reconexión RDP o un cambio de escritorio virtual.

- 🛡️ Auditoría de seguridad: la lista de modelos y los nombres de los agentes proceden del mismo catálogo de inicio que el resto de la aplicación, de modo que las ejecuciones anteriores muestran el nombre actual de cada agente.

- 🩹 Otras correcciones: los mensajes de error de la conversación se alinean con la columna central; los dos diálogos de inicio se pueden cerrar siempre y un inicio confirmado que falló se puede cancelar; los mensajes y los mensajes en cola indican quién los envió; una cola larga se desplaza dentro de una altura fija; los anclajes HTML vacíos ya no muestran una marca al editar un documento; y el filtro de estado del teléfono incorpora las sesiones que pasan a cumplirlo después.

---

## v0.2.2 — 2026-09-15

- ⏳ Continuación automática tras un límite de uso, desactivada por defecto en los ajustes: cuando Claude o Codex se detiene por un límite de 5 horas o semanal, la sesión se reanuda sola en cuanto se restablece el límite. Sobre la zona de escritura aparece un aviso con la hora del restablecimiento y un botón «Cancelar»; la espera sobrevive al reinicio de la aplicación y termina al enviar un mensaje, revertir, vaciar la sesión o desactivar el ajuste.

- 🪟 Windows: las instalaciones en un clic de OpenCode, Grok y Crush pasan `--allow-scripts`, de modo que npm genera el ejecutable; Cursor, OMP y Antigravity se buscan en su carpeta de instalación real bajo `%LOCALAPPDATA%`, y OMP también respeta `PI_INSTALL_DIR`.

- 🪟 Windows: los archivos que aún se están descargando o copiando dejan de contar como instalados, la tarjeta de instalación espera a que la instalación termine de verdad antes de informar del éxito, y una ruta guardada obsoleta se sustituye por la recién localizada.

- 🧩 Vista de conversación: el razonamiento, las llamadas a herramientas y las respuestas intermedias de un turno pueden ocultarse con «Ocultar pasos» para dejar solo la respuesta final; la barra de herramientas oculta o muestra todos los turnos a la vez, y la búsqueda despliega un turno oculto para localizar una coincidencia.

- 🔐 El botón de permisos ahora muestra el modo con el que se inició la sesión en lugar de «Permisos actuales sin confirmar»; la línea duplicada de «ajuste de inicio» ha desaparecido, y una sesión iniciada con las confirmaciones omitidas vuelve a resaltarse en la barra de estado.

- 🔐 Una sesión sin permisos propios sigue el valor predeterminado global de su tipo de agente en ambas vistas, y editar una sesión ya no convierte el valor heredado en una elección fija de la sesión.

- ↩️ Las páginas de la base de conocimientos incorporan un botón «Subir un nivel»: una entrada vuelve a su sesión, luego a su proyecto y después al inicio, y las sesiones archivadas, las notas y las carpetas suben igual.

- 🌱 Una sesión hija se abre en la misma vista que su sesión principal: desde una sesión en vista de conversación la tarea llega como primer mensaje y las imágenes como adjuntos; desde una sesión en terminal arranca en el terminal con la tarea pasada como argumento de inicio.

- 🎨 macOS: la ventana y la franja de la barra de título toman los colores del tema antes de que aparezca la ventana, y cambiar de tema en marcha los redibuja de inmediato.

- 🗜️ Las sesiones de Claude conservan en el historial de lectura los turnos anteriores a una compactación, mientras que revertir sigue descartando la rama abandonada.

- 🔎 La búsqueda en la conversación mantiene fija la coincidencia seleccionada mientras se carga historial anterior; Intro pasa a la coincidencia anterior y Mayús+Intro a la siguiente.

- 🩹 Volver a una conversación detenida en mitad de su historial restaura la posición de lectura en lugar de mostrar el panel en blanco.

- 🧭 La información de diagnóstico del catálogo de modelos solo aparece al abrir el menú de modelos con la tecla Opción pulsada; las líneas de error se alinean con el texto de los mensajes y la base de conocimientos en inglés muestra «Archived Sessions».

---

## v0.2.1 — 2026-09-14

- 🔎 La página de inicio de la base de conocimientos incorpora un cuadro de búsqueda: una consulta busca a la vez en el conocimiento de sesión y en las notas locales, y agrupa los resultados por origen. Las coincidencias exactas van primero; la búsqueda aproximada (abreviaturas, subsecuencias y erratas) solo se activa cuando no hay ninguna coincidencia exacta, y las consultas en chino siguen siendo por subcadena.

- 🗂️ Las sesiones archivadas pasan a la base de conocimientos. El botón de la barra lateral salta aquí en lugar de abrir su propio panel, el árbol añade una raíz «Sesiones archivadas» agrupada por su proyecto original, y el área principal lista todas las sesiones archivadas con restauración, reorganización, exportación y eliminación en el sitio.

- 🔍 Las sesiones archivadas tienen su propia búsqueda de texto completo: los resultados se agrupan por sesión con el número de coincidencias, y el panel de vista previa recorre cada coincidencia con el mismo resaltado que la búsqueda global. Al abrir una sesión se alterna entre su conversación y sus entradas de conocimiento.

- 🤖 Vista de conversación: cuando falta el ejecutable de un agente, aparece una guía de instalación bajo el mensaje en lugar del error de inicio sin procesar, y «Instalar ahora» cambia a la vista de terminal para ejecutar el comando recomendado. Si el agente está instalado fuera del PATH, la guía acepta directamente la ruta del ejecutable, con selector de archivos del sistema en el escritorio.

- 🩹 Se detectan las instalaciones que parecen presentes pero están rotas: un contenedor de npm global cuyo destino se eliminó o sustituyó se considera no instalado, y una ruta configurada que apunta a ese contenedor abre la guía de instalación sin reescribir tu ajuste.

- 🔐 La entrada del modo de permisos muestra directamente el modo elegido, y su menú marca en cada fila si la opción ya está activa o espera al siguiente turno.

- 🔽 Todos los desplegables usan ahora el componente Select integrado en lugar del control nativo, por lo que se ven igual en macOS 15 y 26, sin la superposición del control del sistema.

- ℹ️ Los proyectos y las colecciones tienen un diálogo de información en su menú contextual.

- 🪟 Windows: los subprocesos que inicia la vista de conversación (agentes, catálogo de modelos y comprobaciones de git) ya no parpadean ventanas de consola.

- 💡 El botón de comentarios de la barra de título pasa después del botón de compartir.

---

## v0.2.0 — 2026-09-13

- 📱 Aplicación VelaTerm para iOS y Android (versión preliminar): conéctate a una máquina por SSH o URL tras confirmar la huella del host, carga la interfaz remota completa dentro de la aplicación, rellena los datos de conexión escaneando un código QR e inicia sesión en una cuenta remota.

- 🛡️ Auditoría de código experimental: inicia desde el menú contextual de un proyecto una auditoría de todo el repositorio, de un directorio o archivo, o de los cambios sin confirmar del árbol de trabajo, con Codex o Claude Code; revisa los hallazgos contra el código fuente y exporta un informe en Markdown o JSON.

- 📓 Base de conocimientos local: abre una carpeta de notas Markdown desde el panel derecho, edita las notas en el editor WYSIWYG, busca por ruta y en texto completo, gestiona etiquetas y favoritos y recupera notas eliminadas de la papelera. Los agentes pueden consultar las notas con `vkb`; las importaciones de carpetas se ejecutan en segundo plano y dejan un registro que puedes expandir, revisar o cancelar.

- 🤖 Sesiones de planificación y ejecución: `vspawn --plan-execute` abre una sesión de planificación que divide la tarea en sesiones de ejecución; `vflow` propone la división, `vtell --report` devuelve el resultado de cada ejecución para su aceptación y una reelaboración reutiliza la sesión de ejecución original. Los worktrees pueden compartirse entre todos los roles o crearse por sesión.

- 🔗 Los proyectos y sesiones compartidos ahora cargan la interfaz real del host detrás de la URL compartida, reenviada por un túnel saliente y limitada al proyecto o la sesión autorizados. Los dispositivos y las autorizaciones se gestionan desde las páginas de cuenta.

- 🧠 La base de conocimientos de sesión admite grupos de proyectos, sesiones y entradas con arrastrar y soltar, cambio de nombre y eliminación; una nueva organización de la misma sesión sustituye a la que estaba en cola.

- 💬 Vista de conversación: los historiales largos se cargan página a página hasta el primer mensaje, todos los agentes compatibles con el motor de conversación (incluido OMP) abren la vista de conversación por defecto y los turnos consecutivos de un mismo agente comparten una sola fila de autor.

- ⌨️ Terminal: las sugerencias nativas del shell ofrecen autocompletado con Tab para zsh, bash, fish y PowerShell en macOS y Linux, las flechas siguen recuperando comandos anteriores mientras la lista está abierta y el núcleo del terminal pasa a xterm 6.

- 📊 El panel Info muestra estadísticas del turno actual para Claude, Codex, Grok, OpenCode, Pi y OMP: tokens de entrada y salida, tasa de aciertos de caché, velocidad de generación, llamadas a herramientas y modificaciones de archivos registradas. Los controles menos usados de la zona de escritura pasan a «Más».

- 🔐 La zona de escritura muestra el modo de permisos configurado, el vigente y el pendiente, y pide confirmación cuando hace falta reiniciar para aplicar un cambio.

- 🔔 Las notificaciones muestran el nombre de la sesión y un resumen breve; al hacer clic se abre la sesión, y la aplicación móvil puede recibirlas mediante el servicio de notificaciones push del sistema.

- 🌐 La lista de modelos de Claude ahora proviene del catálogo de modelos publicado en el sitio, se guarda en caché local y se actualiza cada seis horas, combinada con los modelos que informa la CLI.

- 🧵 Las flechas arriba y abajo de la zona de escritura recuperan tus mensajes anteriores, incluidos los que están en cola o pendientes de confirmación, y restauran al final el borrador sin enviar.

- ↩️ Al revertir un mensaje, sus imágenes vuelven a la zona de escritura para poder enviarlas de nuevo.

- 🔑 Los agentes iniciados en macOS heredan el entorno completo del shell de inicio de sesión, por lo que también se encuentran las herramientas instaladas fuera del PATH predeterminado.

- 💡 Una entrada de comentarios en la barra de título abre la página de comentarios.

- 🕹️ Una entrada del Centro de juegos en la barra de pestañas abre el centro de juegos del sitio (PIXEL WING); en el escritorio, el juego se abre en el navegador integrado.

---

## v0.1.108 — 2026-09-08

- 💬 Vista de conversación experimental para Claude, Codex y OpenCode, con respuestas en streaming, razonamiento, detalles de herramientas, permisos y formularios de preguntas. Las configuraciones nuevas siguen usando la vista de terminal por defecto.

- 🎛️ Los controles de conversación ofrecen ajustes de modelo, cola de mensajes, intervenciones, autocompletado de archivos e imágenes adjuntas según las capacidades del motor.

- 🔎 La búsqueda en conversaciones, los enlaces de archivos, las acciones sobre imágenes y las fuentes independientes facilitan la lectura. El historial remoto se carga progresivamente y los detalles de herramientas, bajo demanda.

- 📨 Los recibos de envío permiten cotejar los resultados tras una desconexión. Los envíos inciertos quedan pendientes de confirmación, sin reenvío automático.

- 🤝 Los nuevos comandos `vrefer` y `vsearch` leen y buscan en otras sesiones; `vrefer --ask` delega la lectura a un agente. `vorch` y `vstat` añaden coordinación de múltiples agentes y consulta de estados.

- 🧠 Las entradas de memoria admiten etiquetas, edición directa, protección de cambios sin guardar y ajustes de organización. Mejoran los filtros y la selección al importar historiales, así como la navegación del grafo.

- 🌐 El cliente de uso compartido público admite vinculación de cuentas y dispositivos, acceso limitado a sesiones AI y conexiones de retransmisión cifradas. No admite todavía imágenes subidas por invitados ni formularios MCP complejos.

- 🔄 Los ID de reanudación de Codex se comprueban con el historial persistido. Si se confirma que falta, se muestra un error explícito en vez de iniciar una sesión vacía; sigue siendo posible reconectar terminales en ejecución.

- ↩️ La reversión depende del motor: Codex solo restaura la conversación, sin restaurar archivos. El manejo del alcance y las comprobaciones previas del directorio en OpenCode siguen teniendo limitaciones conocidas.

- 📦 Los cambios de versión conservan las versiones bloqueadas de las dependencias. La validación de instalación y actualización multiplataforma y de integración AI real sigue pendiente; las comprobaciones automatizadas no la sustituyen.

---

## v0.1.107 — 2026-09-05

- 🧠 Memoria global (experimental): Claude o Codex convierte tus conversaciones en una wiki compartida organizada por temas
- 🕸️ Grafo de código (experimental): indexa un directorio de trabajo, recorre las relaciones entre símbolos y vincúlalas con tus memorias
- 🔎 Los agentes pueden consultar código y memoria durante una sesión con `vknowledge`
- 🖥️ SSH puede reflejar la app de escritorio remota: las mismas pestañas, divisiones y sesión activa en ambas máquinas
- 🪟 Las máquinas remotas por SSH ya pueden ser Windows
- 📥 Importa las sesiones de Codex, Claude y OpenCode que ya existen en una carpeta de proyecto
- 🤖 Nuevo agente: OMP
- 🎚️ `vspawn` permite elegir el modelo y el nivel de razonamiento de una sesión hija
- 🌿 `vspawn-tree` funciona en colecciones, y las sesiones de worktree en ejecución muestran un icono de rama
- 🔤 La búsqueda prioriza palabras completas y resalta exactamente lo que ha encontrado
- 🖱️ Cierra una pestaña con un clic del botón central
- ⌨️ Navegadores en macOS: ⌘D y ⌘⇧D dividen el panel
- 💬 La escritura con IME ya no desplaza la vista y el cursor se ve mientras compones
- 🪓 Una división hecha desde el menú solo afecta a la ventana con el foco, y cada división queda registrada en `logs/split.log`
- 📁 El selector de carpetas remoto conserva la ruta que escribes
- ℹ️ Panel de información: hora de inicio y tiempo en ejecución en la misma fila, con la carga media en macOS
- 🔑 Los campos de contraseña ya no muestran el botón de revelar propio del navegador

---

## v0.1.106 — 2026-09-02

### Espacio de trabajo

- **La barra lateral ahora tiene colecciones: contenedores de nivel superior que no están vinculados a una carpeta en el disco.** Hay sesiones que nunca han pertenecido a un repositorio, como sesiones remotas, páginas del navegador o una terminal abierta solo para probar algo. Hasta ahora, el único lugar donde meterlas era dentro de un proyecto, donde quedaban fuera de lugar. Una colección es una fila independiente con nombre propio y sin ninguna carpeta asociada en disco. Al no haber una raíz de proyecto, las acciones que no aplican se ocultan desde el principio en vez de fallar después: "New Worktree Session", "Move to Worktree…" y el selector de worktrees en el cuadro de diálogo de nueva sesión quedan ocultos en las colecciones en lugar de dar error al hacer clic. En su lugar, el cuadro de diálogo de nueva sesión incluye un campo Working directory que apunta por defecto a tu carpeta personal, acompañado de un selector de carpetas nativo en la app de escritorio. Las sesiones que se abran sin especificar un directorio ahora arrancan en tu directorio personal en vez de heredar la ruta de inicio de la aplicación (que en macOS era `/`).

- **El panel de recursos (Resources) muestra todo el equipo, no solo la sesión actual.** Antes, el panel solo mostraba el uso de CPU y memoria del árbol de procesos de la propia sesión, por lo que resultaba imposible saber si la sesión estaba consumiendo todos los recursos o si la máquina ya venía saturada. Ahora el panel se divide en THIS SESSION y SYSTEM. La sección SYSTEM incluye barras de progreso para CPU, memoria y swap (que cambian a color ámbar al superar el 70 % y a rojo por encima del 90 %), junto con una métrica específica de cada plataforma: presión de memoria en macOS, promedios de carga normalizados por cantidad de núcleos en Linux, y ninguna en Windows (que no dispone de una métrica equivalente). Si solo te interesa tu sesión, desmarca "system" en la cabecera del panel: las filas del sistema desaparecerán y el muestreo en segundo plano se detendrá por completo. Esta preferencia se guarda y se aplica al panel de todas las sesiones.

- **Windows y Linux incorporan una barra de menús accesible con la tecla Alt.** macOS cuenta con menús nativos del sistema, pero en Windows y Linux antes solo se podía acceder a los ajustes, la comprobación de actualizaciones y la división de paneles a través del icono de la barra de título. Ahora, pulsar y soltar la tecla Alt por sí sola muestra u oculta una barra de menús. Las combinaciones de teclas con Alt se mantienen intactas: como Alt funciona como prefijo Meta en las terminales, atajos como Alt+B y Alt+F se siguen enviando directamente a la shell; únicamente la pulsación de Alt en solitario, que no envía nada a la terminal, abre el menú. La barra incluye File (ajustes, buscar actualizaciones), Terminal (nueva terminal, dividir a la derecha, dividir abajo) y Help (sitio web, feedback, compartir), mostrando los atajos según tus combinaciones de teclas personalizadas. Permite navegar tanto con el ratón como con el teclado: pulsar Esc cierra el menú abierto, luego oculta la barra y devuelve el foco a la terminal.

- **La opción "Clear notification badges" del menú borra un contador que no se podía quitar de ninguna otra forma.** El contador del dock suma las sesiones no leídas y las tarjetas de confirmación de spawn sin responder. Sin embargo, el botón de limpiar de la barra lateral solo aparecía cuando había sesiones no leídas y solo servía para limpiar esas sesiones. Si quedaba suelta una tarjeta de confirmación sin responder —por ejemplo, de una ventana cerrada o de una sesión eliminada donde la tarjeta ya no se podía dibujar—, el icono del dock se quedaba mostrando un 1 sin forma de quitarlo. La nueva opción de menú marca todas las sesiones como leídas, resuelve como rechazadas todas las solicitudes de spawn pendientes y restablece directamente a cero el contador del sistema operativo. Esto es especialmente importante en macOS, que conserva el contador del dock incluso tras reiniciar la aplicación.

- **La ventana ya no se congela cuando los comandos leen archivos o esperan respuestas del sistema.** Los comandos síncronos de Tauri se ejecutan en el hilo principal, congelando la interfaz de usuario durante todo el tiempo que tardan en completarse. Una auditoría de los 43 comandos reveló que 13 de ellos realizaban operaciones de entrada/salida (I/O) en el hilo principal: solicitar permisos de notificación en macOS podía bloquear la app hasta un minuto esperando la confirmación del usuario; leer grabaciones de sesiones largas recorría decenas de megabytes en bloques de 64 KB; pegar capturas de pantalla escribía directamente en el disco; y consultar el directorio de trabajo de una sesión ejecutaba `lsof`. Ahora los 13 se ejecutan fuera del hilo principal. El procesamiento de las pulsaciones de teclas se mantiene en la ruta rápida (fast path), sin cambios.

### Acceso remoto

- **Los hosts ahora muestran qué clientes remotos están conectados.** Dado que la duplicación de sesión (mirroring) es bidireccional, un cliente remoto puede reorganizar la distribución del host; sin embargo, antes el host no tenía forma de saber si había alguien conectado, y mucho menos quién. La barra de título ahora muestra un distintivo "Mirrored by N"; al hacer clic en él se despliega una lista de los clientes conectados con sus nombres, direcciones IP y marcas de tiempo de conexión. El nombre del cliente se envía durante el protocolo de enlace cifrado (handshake), recurriendo a "Unnamed" si no se proporciona ninguno.

### Agentes de IA

- **El consumo de la cuenta se consulta una sola vez por equipo en lugar de hacerlo por cada sesión.** Los límites de uso pertenecen a la cuenta, pero antes el panel Info de cada sesión los consultaba por su cuenta. Tener diez sesiones abiertas provocaba diez solicitudes duplicadas para obtener los mismos datos, lo que solía activar los estrictos límites de tasa de Claude y dejar el panel en blanco. Ahora, un servicio centralizado de consulta en segundo plano mantiene una única instantánea de la que leen todas las sesiones, emitiendo actualizaciones cada vez que los datos cambian. Solo consulta los proveedores que usas activamente —detectados por la presencia de `~/.claude`, `~/.codex` o `~/.grok/auth.json`—, sin acceder al llavero del sistema para evitar avisos de permisos innecesarios. Las instantáneas se guardan en disco para que el panel muestre los datos en caché de inmediato al reiniciar, sin esperar a la primera consulta. Además, al reactivar el portátil tras suspenderlo, los datos se actualizan enseguida en vez de esperar al siguiente intervalo programado. Los fallos de consulta consecutivos aplican un retardo exponencial (backoff) de hasta 8x. En Settings, el intervalo de actualización ahora incluye un interruptor explícito de activación/desactivación junto al campo de tiempo, reemplazando el ambiguo ajuste de 0 segundos.

- **Las actualizaciones de consumo fallidas ahora se marcan claramente como desactualizadas en lugar de parecer al día.** Antes, cuando una solicitud de actualización fallaba, el cliente devolvía en silencio la copia en caché mientras registraba la operación como exitosa: avanzaba la marca de tiempo, borraba los errores y reiniciaba el temporizador de backoff. Al alcanzar los límites de tasa, esto hacía que el panel pareciera funcionar con normalidad a pesar de que seguía reintentando cada cinco minutos. Ahora, las solicitudes fallidas conservan los valores en caché pero muestran un distintivo ámbar `stale` junto al proveedor. Al pasar el cursor sobre él, se muestra una ventana emergente (tooltip) con la hora del fallo, el motivo del error y la antigüedad de la lectura mostrada. La marca de tiempo "Updated" refleja la última sincronización correcta y se respetan las cabeceras retry-after de los servidores con límite de tasa, aunque las actualizaciones manuales seguirán ejecutándose de inmediato.

- **Las cuotas semanales por modelo aparecen en el panel de consumo.** Además de los límites generales de la cuenta para 5 horas y 7 días, el endpoint de consumo reporta cuotas semanales específicas por modelo. Antes se ignoraban, por lo que los usuarios que trabajaban con modelos como Fable solo podían ver el consumo total de la cuenta en lugar de la cuota restante específica para ese modelo. Estas filas de desglose ahora se muestran debajo de los totales, organizadas por familia de modelos.

- **Reabrir una sesión de OpenCode restaura su historial de chat.** Antes, la aplicación comprobaba si una sesión seguía existiendo mediante `opencode session list`, que solo busca sesiones dentro del directorio de trabajo actual. Dado que la app de escritorio se ejecuta desde su directorio de lanzamiento (`/` en macOS), nunca encontraba los ID de sesiones existentes, lo que provocaba que los intentos de reanudar abrieran silenciosamente sesiones en blanco. Ahora las comprobaciones consultan directamente el ID de la sesión específica y solo la consideran eliminada si el comando lo confirma de forma explícita. Cualquier otro error se trata como indeterminado y la app procede con la reanudación, evitando la pérdida accidental del historial de la sesión.

### Interfaz

- **Al escribir con un editor de métodos de entrada (IME) ahora se muestra el texto en composición.** En las tres plataformas, el texto compuesto mediante un IME (como Pinyin o Kana) era invisible hasta que se confirmaba pulsando Enter. Esto se debía a una regla CSS personalizada: xterm renderiza el texto en composición dentro de un contenedor superpuesto sin restricciones, donde la restricción `right` que añadimos (pensada para ajustar líneas largas de terminal) daba un ancho de cero, mientras que `overflow: hidden` cortaba el texto por completo. Esa regla se ha eliminado, y lo único que se sigue sobrescribiendo es el esquema de color predeterminado en negro sobre blanco del overlay, que no encaja con ningún tema. Al mismo tiempo se solucionó un problema relacionado: xterm agrandaba su textarea oculta para que coincidiera con el tamaño del overlay y posicionar los candidatos del IME, pero nunca volvía a reducirla, dejando una capa invisible que bloqueaba los clics del ratón y la selección de texto en esa línea.

- **En macOS, los atajos con Ctrl ya no activan las combinaciones de teclas de Command.** Antes, las comprobaciones de teclas modificadoras trataban Cmd y Ctrl como intercambiables en macOS. Como resultado, los atajos estándar de terminal entraban en conflicto con los de la aplicación: Ctrl+D (EOF) dividía paneles, Ctrl+W (borrar palabra) los cerraba, y combinaciones como Ctrl+F, Ctrl+T y Ctrl+O eran interceptadas por la aplicación. Al abrirse nuevos paneles de forma silenciosa, a menudo parecía que aparecían de la nada. Ahora, la aplicación de escritorio en macOS usa exclusivamente Cmd para los atajos, mientras que las demás plataformas (y los navegadores web conectados a un Mac) usan exclusivamente Ctrl.

- **Los menús desplegables ya no se vuelven a abrir de inmediato al seleccionar una opción.** Un elemento `<label>` envolvente propagaba los clics de toda la fila hacia el botón activador, lo que provocaba que el menú desplegable se reabriera en cuanto se seleccionaba una opción.

### Correcciones

- **Linux: la AppImage vuelve a arrancar.** La AppImage de la versión 0.1.105 fallaba en Ubuntu 22.04 antes de mostrar ninguna ventana, al ser incapaz de iniciar los procesos auxiliares de WebKit. Las herramientas de empaquetado de AppImage reescriben las rutas literales `/usr` en los binarios como `././` (así conservan la misma longitud y se pueden parchear in situ), lo que requiere que el script de inicio cambie el directorio de trabajo a `$APPDIR/usr` para que esas rutas se resuelvan. En la 0.1.105, nuestro script de inicio personalizado —introducido para evitar que las variables de entorno se filtraran a las shells secundarias— transfería las variables de entorno pero omitía este cambio de directorio. Ahora el lanzador cambia de directorio correctamente, respaldado por una aserción en tiempo de compilación para prevenir regresiones. Las versiones 0.1.104 y anteriores no estaban afectadas.

- **Windows: las notificaciones vuelven a emitir sonido y hacer clic en una te lleva a su sesión.** Antes, las notificaciones de Windows enviaban un identificador de sonido de macOS al plugin de notificaciones. Al no poder interpretar ese nombre, se convertía en silencio, por lo que las notificaciones en Windows nunca emitían sonido sin importar la configuración. Las notificaciones de Windows se habían desviado a través del plugin hace dos versiones como solución temporal a un fallo solucionado en esa misma versión. Ese desvío se ha deshecho: las notificaciones vuelven a transmitirse por el canal nativo, recuperando tanto el sonido como la navegación a la sesión al hacer clic.

- **Windows: los caracteres de dibujo de cajas y bloques quedan alineados.** Antes, los marcos de la terminal se renderizaban con una fuente tipográfica alternativa proporcional, lo que generaba distorsiones visuales. Incluso tras solucionar eso, los caracteres de bloque (usados en las barras de progreso y en la mascota de Claude Code) seguían dejando una fina rendija vertical en el borde derecho de cada celda, ya que la fuente alternativa era ligeramente más estrecha que la fuente principal de la terminal. Ahora, ambos rangos de caracteres utilizan un subconjunto integrado de JetBrains Mono, garantizando una coincidencia exacta en el ancho de los glifos.

- **Windows: `git` funciona en la variante completa de Git Bash.** Aunque la instalación completa de Git Bash se completaba con éxito, al ejecutar `git` en la terminal aparecía el error "command not found", solicitando repetidamente al usuario que instalara la versión completa de Git Bash. Git Bash solo añade `mingw64/bin` al `PATH` cuando se le indica en qué árbol de directorios se está ejecutando. Como nuestro lanzador de shell omitía ese parámetro, no se encontraban los binarios ubicados exclusivamente en ese directorio, como `git` y `curl`. Además, el diálogo de descarga ahora selecciona el instalador correspondiente a la arquitectura del sistema anfitrión en vez de elegir 64 bits por defecto.

- **Windows: eliminar un worktree funciona y reinstalar los hooks de Kiro ya no los duplica.** Antes, los comandos para eliminar worktrees se ejecutaban desde el propio directorio del worktree de destino. Como Windows impide eliminar el directorio de trabajo actual de un proceso activo, la operación fallaba sistemáticamente con un error de permisos; ahora los comandos de Git se ejecutan desde la raíz del repositorio principal. Además, la lógica de detección de hooks comparaba rutas con barras inclinadas hacia adelante (/) con las barras invertidas de Windows (\), lo que hacía que la comprobación fallara y se añadieran entradas duplicadas en cada reinstalación.

---

## v0.1.105 — 2026-08-26

### Espacio de trabajo

- **El estado de las sesiones, las marcas de no leído y si una sesión sigue en marcha los decide ahora el backend, y todos los clientes ven la misma respuesta.** Antes cada cliente lo deducía de lo que le tocaba observar, así que «no leído» significaba en realidad «no leído en esta ventana»: leer una sesión en el navegador dejaba la copia del escritorio sin leer, y el filtro de estado dinámico devolvía esa fila a la lista compartida, donde ni siquiera «Actualizar estado» conseguía quitarla. Además, los eventos de estado solo se registraban una vez que el propio cliente había creado alguna sesión, así que un navegador recién conectado mostraba puntos en las sesiones que él había abierto y nada en el resto. Ahora el backend mantiene un único registro autoritativo por sesión —el agente, su estado, si su proceso sigue vivo y la marca de no leído—, responde a una consulta en bloque cuando un cliente se conecta o se reconecta, y difunde cada cambio a todo el mundo. Los clientes informan de lo que observan; el backend saca las conclusiones. Leer una sesión en el teléfono borra la marca en el escritorio, un navegador recién conectado muestra los puntos correctos en sesiones que nunca ha abierto, y las reglas de arbitraje que vivían en el frontend se han mudado con sus motivos y sus pruebas: un hook que ya ha informado una vez bloquea cualquier cosa deducida de la salida en bruto, la salida continua solo implica «ocupado» en una sesión que tiene agente, no es Codex y aún no tiene un informe autoritativo, y una retención de 1200 ms evita que una tarea recién iniciada la cancele un evento de finalización que llega justo detrás. La lectura de pantalla es la excepción, porque necesita la cuadrícula ya renderizada y esa solo existe en un cliente: el cliente que manda sobre el tamaño del terminal informa de lo que ha leído, y el backend decide si lo acepta; un informe de cualquier otro cliente se rechaza. Si algo de esto se tuerce, poner `vlx-arbitration` en `frontend` dentro de localStorage devuelve el arbitraje a la antigua cadena del frontend.

- **Reiniciar una sesión ya no cierra su pestaña en el otro cliente.** `pty://killed` no llevaba ningún dato, así que el otro lado no podía distinguir un reinicio de un cierre: lo trataba todo como un cierre, quitaba el panel y luego reflejaba de vuelta esa disposición. Ahora el evento dice qué cliente ha matado el proceso y por qué, de modo que un reinicio conserva el panel y espera a que llegue el proceso nuevo. Un motivo ausente o desconocido sigue contando como cierre: dejar abierto el panel de una sesión que no va a volver pone un terminal muerto en pantalla, y eso es peor que cerrar uno que estaba a punto de reiniciarse.

- **Un navegador que se conecta a un escritorio que acaba de restaurar su espacio de trabajo ya no arranca de verdad todas las sesiones.** Restaurar un espacio de trabajo dibuja tarjetas de marcador de posición en lugar de lanzar procesos, pero esa decisión solo existía en el escritorio; el navegador seguía la disposición reflejada, no podía distinguir «no está en marcha» de «está en marcha, solo que nunca se abrió aquí», y montaba los terminales —y montar uno es lanzarlo—. Ahora una hoja que llega con la disposición de otro dibuja un marcador de posición cuando el backend dice que no hay ningún proceso detrás. Abrir una sesión tú mismo sigue siendo la intención de arrancarla, y un terminal que estás mirando en ese momento nunca se sustituye por una tarjeta cuando su proceso termina, porque quizá aún quieras leer lo que imprimió.

- **Un cambio en los ajustes llega de inmediato a los demás clientes.** El backend siempre ha guardado los ajustes autoritativos, pero los cambiaba sin avisar a nadie, así que el otro cliente se enteraba en su siguiente arranque. En una conexión remota eso es más que una discrepancia visual: desactivar «ampliar el filtro de estado dinámicamente» en un cliente no servía de nada mientras otro seguía añadiendo filas a la lista compartida, y el cliente con el límite de pestañas vivas más bajo iba expulsando las pestañas de fondo de todos los demás. Ahora escribir un ajuste difunde qué clave ha cambiado, y cada cliente vuelve a leerla por el mismo camino que usa al arrancar, así que siguen aplicándose las reglas que ocultan los valores protegidos a los clientes remotos: la difusión lleva solo nombres de clave, nunca valores. Los teléfonos, que antes ni enviaban ni recibían ajustes, participan también ahora.

- **Un navegador espera a la disposición reflejada antes de restaurar la suya.** Una ventana remota tiene dos fuentes de disposición —la de su propio localStorage y la que el anfitrión le envía en modo espejo—, y la que llegaba primero acababa sobrescrita por la otra. Poner la local por delante costaba más que un parpadeo: montar una hoja de terminal lanza un proceso real, y en una sesión cuyo proceso ya no existía eso significaba arrancar un shell que nadie iba a mirar, y que además se quedaba ahí, porque un navegador desacopla sus terminales en lugar de matarlos. Ahora el navegador espera a que se asiente la primera alineación antes de restaurar nada, como mucho dos segundos; si el backend va lento o no responde, recurre a la disposición local en lugar de quedarse con una ventana vacía. Los teléfonos, y cualquier cliente con el reflejo desactivado, pasan de inmediato.

### Acceso remoto

- **Una conexión SSH puede activar el modo espejo para el servicio que arranca.** El interruptor vive en el panel de acceso remoto, pero SSH arranca un servicio sin interfaz en la máquina remota y allí no hay ningún panel donde hacer clic. Mantén pulsada la tecla Option al pulsar «Connect remote» y el formulario de SSH ofrece ahora una casilla «Mirror UI across clients», desactivada de serie. El valor viaja con la conexión y se guarda en la memoria del servicio en lugar de escribirse en la base de datos de la máquina remota: cuando además reutilizas la base de datos del propio escritorio remoto, una conexión SSH no debería cambiarle a nadie un interruptor de su panel sin avisar. La elección se recuerda por host, así que escoger la misma máquina del historial la trae de vuelta. Reutilizar un servicio que ya está en marcha exige ahora que coincidan la versión, el modo de datos y el modo espejo: si difiere cualquiera de los tres se sustituye el servicio antiguo, lo que termina las sesiones que corrían en él, y por eso esta opción está detrás de Option, junto al interruptor de la base de datos.

- **Un cliente que está siendo reflejado lo dice.** Las pestañas y las divisiones de un cliente que sigue a otro se reorganizaban solas sin nada en pantalla que explicara de dónde venía el cambio. Ahora la barra de título lleva un distintivo «Mirrored», con una explicación al pasar el ratón por encima. El anfitrión no lo muestra: él tiene el interruptor.

- **Ahora se pueden mover archivos entre tu equipo y el que ejecuta la terminal.** El acceso remoto mostraba los archivos de la otra máquina pero no ofrecía forma de traerte uno ni de dejar uno allí: la única vía era un comando en la terminal. El panel de archivos incluye ahora Descargar en el menú contextual de un archivo y Subir en su cabecera, y arrastrar archivos desde tu escritorio hasta la fila de una carpeta los envía allí. Ambas direcciones usan la misma conexión autenticada que todo lo demás, así que funciona igual desde un navegador en la red local, desde un teléfono y desde una ventana de conexión remota. Las transferencias avanzan por bloques con una cola de progreso bajo el árbol y siguen en marcha mientras miras otro panel. Descargar es un enlace de descarga corriente, del que se encarga el gestor de descargas del propio navegador: escribe en disco sobre la marcha, muestra velocidad y tiempo restante, y puede pausarse y reanudarse, con cualquier tamaño de archivo y en cualquier navegador, también en el teléfono. El enlace lleva un vale emitido para ese único archivo y válido unos minutos, porque este servidor guarda las credenciales en una cabecera y un navegador que abre un enlace no envía ninguna. Una subida se escribe con un nombre temporal y solo se renombra al terminar, de modo que una transferencia interrumpida nunca deja un archivo a medias donde debería haber uno completo; un nombre ya ocupado se rechaza antes de mover nada. Las subidas muestran su velocidad y el tiempo restante, y sobreviven a una caída de la conexión: un bloque fallido espera y reintenta durante alrededor de un minuto, preguntando al servidor hasta dónde llegó realmente su archivo temporal en vez de reenviar un bloque que quizá ya llegó. Al rendirse conserva esos bytes: arrastra el mismo archivo a la misma carpeta y continúa donde se quedó, incluso tras recargar, porque solo cancelar descarta lo ya subido.

### Agentes de IA

- **Las sesiones de Antigravity y de Copilot toman su nombre del primer mensaje.** Las dos faltaban en el renombrado automático, lo que dejaba filas de «Antigravity 1, 2, 3» en la barra lateral. Los eventos de hook de Antigravity no llevan ningún texto del usuario, solo un id de conversación y una ruta a la transcripción, así que el primer mensaje se lee de la transcripción; el bloque de metadatos que viene detrás se deja fuera del título. Los eventos de Copilot tampoco llevan nombre de evento y se distinguen por su forma, así que un cuerpo con un prompt y sin nombre de herramienta se toma ahora como un envío, lo que deja fuera —correctamente— tanto el prompt con el que se inicia una sesión como las llamadas a herramientas.

- **Una sesión de Antigravity vuelve a abrirse con su historial.** Reanudar necesita el id de conversación, y el analizador que extrae el id de sesión de los argumentos de inicio no reconocía cómo lo escribe Antigravity, así que `--conversation=<id>` nunca tenía a qué apuntar y toda sesión reabierta salía vacía.

- **`vspawn --yes` crea la sesión sin la tarjeta de confirmación.** Quien tenga activado «confirmar antes de crear una tarea hija» tenía que pasar por una tarjeta por cada sesión hija de una tanda. El indicador —que también se escribe `-y` o `--no-confirm`— se salta la tarjeta en esa llamada concreta e inicia la sesión con los ajustes predeterminados. No cambia el ajuste en sí, así que la siguiente vez que se cree una sin él vuelve a preguntar.

- **El campo de modelo de la tarjeta de tarea hija acepta lo que escribas.** Era un desplegable a secas, así que solo podían elegirse los modelos de la lista, y un agente entiende muchos más identificadores que esos: nombres con fecha como `claude-opus-4-6`, nombres con prefijo de proveedor, alias configurados en local. Ahora es un campo de texto con los modelos conocidos colgando de un desplegable a su lado, a modo de atajo. La lista es una sugerencia, no una lista blanca: se pasa exactamente lo que escribas, un campo vacío significa que no se pasa ningún `--model`, y la lista se filtra mientras escribes y se pliega cuando un identificador personalizado no coincide con nada.

### Interfaz

- **Un solo desplegable, usado en todas partes.** Los desplegables repartidos por la aplicación se habían copiado del mismo código más de una docena de veces y luego habían ido separándose: tres fondos de panel, cuatro sombras, tres colores al pasar el ratón por encima, alturas de disparador de 26, 28 y 32 píxeles, y marcas de selección en las filas elegidas que solo necesita una lista de selección múltiple. Ahora un único componente sostiene los selectores de rama al fusionar, los de idioma, shell predeterminado y fuente en los ajustes, el selector de agente, los diálogos de worktree, el tipo de agente en las sesiones nuevas y en las restauradas, y el último select nativo que quedaba en el modal de formulario. Trae además control por teclado, que ninguno tenía: flechas para moverse, Intro para elegir, Escape para cerrar sin cerrar el diálogo que hay detrás, Inicio y Fin para saltar. Los dos menús de la barra de estado se comportan igual que antes, y el filtro de estado de la barra lateral conserva sus marcas, porque ahí sí es selección múltiple.

- **La contraseña del panel de acceso remoto puede mostrarse.** Era un campo de contraseña pelado, así que no podías ver lo que habías escrito; el botón con forma de ojo existía, pero solo dentro del archivo del propio panel de conexión. Ahora los dos comparten un mismo componente, y el estado de mostrado se restablece al cerrar el panel.

- **El selector de IP ya no parece un control del sistema.** Era un select nativo, y WKWebView lo viste con adornos del sistema que quedan mal sobre un panel oscuro: la misma queja que la de los desplegables sustituidos más arriba. Ahora usa el componente compartido, y su etiqueta se acorta a «IP», porque el texto que tiene al lado ya dice para qué sirve.

### Correcciones

- **Buscar actualizaciones consulta al servidor todas las veces.** Un cliente que se quedaba abierto se quedaba clavado en la primera versión que hubiera visto: si había encontrado la 0.1.101, seguía ofreciendo la 0.1.101 después de publicarse la 0.1.104, y «Buscar actualizaciones» solo volvía a abrir el mismo diálogo, porque el código antiguo salía antes de tiempo siempre que hubiera un aviso pendiente. Ahora cada comprobación es una petición de verdad. Una versión más nueva sustituye el aviso que haya en pantalla, la misma versión o una descarga en curso lo dejan en paz, y un servidor que no informa de ninguna actualización retira un aviso que se ha quedado obsoleto: la versión se retiró, o la instalaste tú mientras tanto. El botón «Descargar manualmente» abre ahora la página de descargas del sitio web; antes te entregaba el paquete propio del actualizador, que se desempaqueta en su sitio y no se puede instalar a mano.

- **Ya no aparece una superposición de error a pantalla completa cuando los terminales se destruyen deprisa.** El viewport de xterm programa una sincronización del área de desplazamiento al construirse y otra al reiniciarse, y no cancela ninguna de las dos al destruirse, así que un terminal abierto y cerrado dentro de la misma tarea —que es exactamente lo que hace reconstruir el árbol de sesiones durante una conexión remota— llegaba a ejecutar esas devoluciones de llamada, se encontraba un renderizador ya limpiado y lanzaba un error. El error sale de un temporizador, donde no llegan ni un try/catch ni un límite de error, así que se captura globalmente y se compara de forma estricta: solo se traga como inofensivo, y se anota en el registro de peticiones, aquello cuya pila o mensaje nombre esa sincronización junto con una mención al renderizador o a sus dimensiones. Los fallos de verdad siguen levantando la superposición.

---

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
