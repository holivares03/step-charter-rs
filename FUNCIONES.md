# Lista de funciones: ArrowVortex vs Step Charter

Estado: ✅ hecho · 🟡 parcial · ❌ pendiente

La lista de ArrowVortex sale de los menús, diálogos, atajos y textos de ArrowVortex 1.0.1
(`D:\ArrowVortex-1.0.1-Windows`). Al final están los extras propios y las ideas nuevas.

**Resumen de paridad con ArrowVortex:** 74 ✅ · 8 🟡 · 29 ❌ (111 funciones)

---

## 1. Archivos

| Función | Estado | Notas |
|---|---|---|
| Abrir `.ssc` | ✅ | Incluye split timing y formato StepF2 |
| Abrir `.sm` | ✅ | Se guarda como `.ssc` |
| Abrir `.dwi` | ❌ | |
| Abrir `.osu` / `.osz` (osu!mania) | ❌ | |
| Abrir un audio suelto (`.ogg/.mp3/.wav`) para empezar un simfile nuevo | ✅ | Desde *Abrir*, arrastrándolo o *Archivo → Nuevo simfile desde audio*; crea el `.ssc` junto al audio |
| Guardar `.ssc` | ✅ | Sin pérdidas; `.bak` del original la primera vez |
| Guardar `.sm` | ❌ | |
| Guardar como | 🟡 | Sólo a `.ssc` |
| Archivos recientes | ✅ | Falta "Limpiar lista" |
| Cerrar archivo | ✅ | |
| Simfile anterior / siguiente de la carpeta (Shift+F5/F6) | ❌ | |
| Convertir la música a `.ogg` (oggenc2) | ❌ | |
| Aviso de cambios sin guardar | ✅ | Al abrir otro, cerrar o salir |
| Arrastrar y soltar archivos | ✅ | |

## 2. Charts

| Función | Estado | Notas |
|---|---|---|
| Lista de charts | ✅ | Panel lateral y menú *Chart* |
| Chart anterior / siguiente (F5/F6) | ✅ | |
| Nuevo chart | ✅ | Shift+N; puede copiar las notas o el timing propio del chart actual |
| Borrar chart | ✅ | Ctrl+Supr, con confirmación y deshacer |
| Propiedades del chart (tipo, descripción, dificultad, nivel, autor) | ✅ | Shift+C; el tipo sólo cambia entre estilos con el mismo número de columnas |
| Estimar la dificultad analizando las notas | ❌ | |
| Convertir ITG couples ↔ routine | ❌ | |

## 3. Propiedades de la canción

| Función | Estado | Notas |
|---|---|---|
| Ver título, artista, crédito, música, offset y BPM | ✅ | Panel lateral |
| Editar título, subtítulo, artista, transliteraciones y crédito | ✅ | Shift+P; también el género |
| Editar rutas de música, banner, fondo y CD title | ✅ | Al cambiar la música o el fondo se recargan |
| Buscar audio e imágenes en la carpeta del simfile | ✅ | Lista de los archivos de la carpeta junto a cada campo |
| Display BPM | ✅ | Real, fijo, rango o aleatorio (`*`) |
| Preview de la música (inicio/duración, "desde la selección", reproducir) | ✅ | También "inicio en el cursor" |

## 4. Estilos de juego

| Función | Estado | Notas |
|---|---|---|
| pump-single (5) | ✅ | |
| pump-halfdouble (6) | ✅ | |
| pump-double (10) | ✅ | |
| pump-couple (10) | ✅ | |
| pump-routine (10, 2 jugadores) | ✅ | |
| Estilos dance (single, double, couple, routine, solo) | ✅ | Con noteskin vectorial |
| Estilos configurables (`settings/styles.txt`: pad, mirror, pies) | ❌ | Por ahora están fijos en el código |

## 5. Vista / notefield

| Función | Estado | Notas |
|---|---|---|
| Vista por tiempo (C-mod) | ✅ | |
| Vista por filas (X-mod) | ✅ | |
| Zoom | ✅ | |
| Escala de las notas | ✅ | |
| Reverse scroll | ✅ | |
| Posición de los receptores | 🟡 | Sólo la vertical |
| Líneas de beat y números de compás | ✅ | Respeta las time signatures |
| Cajas de cambios de tempo | ✅ | Todos los tipos de segmento |
| Ayuda de tempo (tempo help) | ❌ | |
| Waveform | 🟡 | Básico, sin opciones |
| Ajustes del waveform (colores, estilo, luminancia, suavizado) | ❌ | |
| Filtro de frecuencias del waveform (superpuesto) | ❌ | |
| Fondo de la canción (stretch / letterbox / crop, opacidad, ocultar) | 🟡 | Sólo crop, opacidad y ocultar |
| Noteskins intercambiables (Classic, metal, simple, pump, bar) | 🟡 | Pump de ArrowVortex integrado, vectorial y los noteskins pump importados del simulador; no lee el formato propio de ArrowVortex (Classic, metal, simple, bar) |
| Noteskin clásico por cuantización | ❌ | |
| Chart preview (vista como en el juego) | ✅ | Vista de juego (Shift+G): `#SCROLLS` y `#SPEEDS` como en StepMania 5; al reproducir, los stops congelan y los warps saltan |
| Minimapa (notas / densidad) | ✅ | Panel derecho a escala fija que se desplaza con el chart; click/arrastre para navegar, rueda por compás, Ctrl+rueda para zoom (Shift+M) |
| Barra de estado (chart, snap, BPM, fila, beat, compás, tiempo, modo) | 🟡 | Se ve todo, pero no se puede configurar qué mostrar |
| Brillo de receptores al pasar las notas | ✅ | También destellan en cada beat al reproducir |
| Notas en warps / fakes marcadas | ✅ | Se dibujan atenuadas |

## 6. Cursor y navegación

| Función | Estado | Notas |
|---|---|---|
| Mover el cursor según el snap (↑ ↓, rueda) | ✅ | |
| Beat / compás anterior y siguiente | ✅ | |
| Inicio / fin del chart | ✅ | |
| Inicio / fin de un stream (Ctrl+↑ ↓) | ❌ | |
| Inicio / fin de la selección | ✅ | Ctrl+Inicio / Ctrl+Fin |
| Cambiar el snap (← →) | ✅ | |
| Snap personalizado (p. ej. 20avos) y fijar/restablecer snap | ❌ | |
| Click para mover el cursor | ✅ | |
| Saltar a la siguiente nota (jump to next note) | ❌ | |

## 7. Edición de notas

| Función | Estado | Notas |
|---|---|---|
| Poner / quitar notas con el teclado | ✅ | Teclas 1–0 |
| Holds manteniendo la tecla y moviendo el cursor | ✅ | |
| Rolls y minas | ✅ | Shift / Alt + tecla (atajos propios) |
| Deshacer / rehacer | 🟡 | Falta que el cursor salte al cambio deshecho |
| Seleccionar región (Tab), antes o después del cursor, todo | ✅ | También selección rectangular con el mouse y Shift+click |
| Seleccionar por tipo (pasos, minas, holds, rolls, fakes, lifts, saltos, notas en warp) | ✅ | Dentro de la región actual o en todo el chart (menú *Editar* y panel lateral) |
| Seleccionar por cuantización (4th … 192nd) | ✅ | |
| Copiar / cortar / pegar / borrar | ✅ | Pega en el cursor reemplazando la región; sirve entre ventanas |
| Copia basada en tiempo (time-based copy) | ❌ | |
| Notas → minas / fakes / lifts, y al revés | ✅ | *Editar → Convertir*; los tokens de StepF2 se actualizan y de fake a nota se conserva el hold |
| Holds → pasos / minas | ✅ | *Editar → Convertir* |
| Holds ↔ rolls | ✅ | Tecla º (bajo Esc, la misma que en ArrowVortex) |
| Cambiar el jugador de las notas (routine) | ✅ | `/` con selección pasa las notas al jugador siguiente; sin selección, cambia el de las notas nuevas |
| Espejo horizontal / vertical / completo | ✅ | Y / T / G, con los mapeos de `styles.txt` |
| Cambiar de lado (mitad izquierda ↔ derecha) | ✅ | De la versión actual de ArrowVortex; en double pasa las notas de un pad al otro sin espejarlas |
| Escalar notas (2:1, 3:2, 4:3, 1:2, 2:3, 3:4) | ✅ | *Editar → Escalar*, respecto a la primera nota; lo que sale de la región se elimina |
| Insertar / borrar beats en el cursor | ✅ | Editor de tempo (Shift+T), en este chart o en todos; mueve notas y cambios de tempo |
| Exportar notas como tabla Lua | ❌ | |
| Copiar información de las notas al portapapeles | ❌ | |

## 8. Tempo y sincronía

| Función | Estado | Notas |
|---|---|---|
| Leer y respetar BPMs, stops, delays, warps (orden de StepMania 5) | ✅ | |
| Ver time signatures, tickcounts, combos, speeds, scrolls, fakes y labels | ✅ | |
| Timing propio por chart (split timing) | ✅ | Se edita, se crea y se descarta desde el editor de tempo |
| Adjust Sync: ajustar offset y BPM inicial, mover el primer beat | ✅ | En el editor de tempo (Shift+T) |
| Detección automática de BPM y offset | ❌ | |
| Doble / mitad de BPM, offset ± medio beat / ± un beat | ✅ | También offset ±1 / ±10 ms |
| Adjust Tempo: insertar y editar BPM, stop, delay, warp | ✅ | Editor de tempo (Shift+T), con lista de segmentos |
| Adjust Tempo SM5: time sig., tickcount, combo, speed, scroll, fake, label | ✅ | Editor de tempo (Shift+T), sección *StepMania 5* |
| Insertar un cambio de BPM que encierre la selección (Ctrl+B) | ✅ | Mismo BPM en los extremos de la selección (o en el cursor): no altera la sincronía |
| Visual sync (arrastrar beats al audio; ancla en cursor o receptor) | ❌ | En ArrowVortex, B / Alt+B (no destructivo / destructivo) |
| Convertir la selección en stop / stutter | ✅ | Menú *Tempo* y editor de tempo; el stutter no desfasa lo que sigue |
| Seleccionar segmentos de tempo por tipo | ❌ | |
| Copiar / pegar datos de tempo | ✅ | Menú *Tempo*: copiar los cambios de tempo de la selección y pegarlos en el cursor |
| Tempo breakdown / stream breakdown (copiar al portapapeles) | ❌ | |

## 9. Audio

| Función | Estado | Notas |
|---|---|---|
| Reproducir / pausar sincronizado | ✅ | |
| Formatos ogg, mp3 y wav | ✅ | También flac |
| Volumen | ✅ | |
| Silenciar (mute) | ❌ | |
| Velocidad de reproducción | ✅ | |
| Tick de beat / tick de notas | ✅ | Sonidos sintetizados |
| Ajuste de latencia de los ticks (tickOffsetMs) | ❌ | |

## 10. Herramientas y configuración

| Función | Estado | Notas |
|---|---|---|
| Dancing Bot (pies sobre el pad, crossovers, footswitches) | ❌ | |
| Generar notas (aleatorias, con espaciado y repetición máxima) | ❌ | |
| Registro de mensajes (F2) | 🟡 | Hay mensajes temporales en pantalla, sin historial |
| Registro de depuración | ❌ | |
| Ventana de atajos (F1) | ✅ | |
| Atajos configurables (`shortcuts.txt`) | ❌ | |
| Ajustes guardados entre sesiones | ✅ | Vista, recientes, snap, volumen, noteskin y minimapa |
| Diálogos fijables (pin) y su posición guardada | ❌ | |

---

## 11. Extras de Step Charter que no tiene ArrowVortex (ya hechos)

- ✅ Formato de notas StepF2/StepP1: `{110}`, `{2m8}`, `{2|n|1|0}`; los tipos desconocidos se conservan.
- ✅ Co-op de StepF2: `pump-single-p` / `pump-double-p` con `#PLAYERS`, bloques `&` (`#SEPARATENOTES`).
- ✅ Colores e insignia con el número de jugador en cada nota co-op.
- ✅ Noteskins pump de StepMania/XSanity/StepF2, con sprite sheets animados, `.redir`, fallback a `default` y noteskins "rhythm" coloreados por cuantización y, en co-op, el noteskin de cada jugador según `#PRELOADNOTESKIN`.
- ✅ Importar noteskins del simulador instalado (*Vista → Noteskin → Importar…*): busca XSanity, StepF2/P1, StepMania, ITGmania u OutFox en las unidades, o una carpeta elegida, y los copia a la carpeta de datos de la app. No se distribuyen noteskins de terceros.
- ✅ Notas, holds y receptores animados y con brillo al ritmo del BPM durante la reproducción.
- ✅ Al pegar notas, los jugadores se adaptan al chart de destino (co-op ↔ un jugador).
- ✅ Selección por jugador (co-op) y por notas especiales de StepF2; los filtros se aplican dentro de una región, así se puede copiar, borrar o espejar sólo las minas, sólo P2, etc.
- ✅ Vista de juego (Shift+G): vista por filas con `#SCROLLS` y el zoom de `#SPEEDS` según la posición, con speed y scroll actuales en la barra de estado.
- ✅ Zonas de gimmicks sobre el chart: warps (morado), fakes (rayado), stops y delays (rojo / naranja, con su duración en la vista por tiempo), transiciones de speed (franja azul a la derecha) y scrolls (franja a la izquierda: cian > 1, verde < 1, amarillo = 0, magenta = hacia atrás).
- ✅ Minimapa con zoom propio para *Notas* y para *Densidad* (Ctrl+rueda) y opción de ver todo el chart en pantalla.
- ✅ Editor de tempo: crear o descartar el timing propio de un chart y lista de todos los segmentos con "ir" y borrar.
- ✅ Propiedades de la canción con editor de los demás tags (los de XSanity/StepF2: `#CATEGORY`, `#SONGTYPE`…).
- ✅ Charts co-op nuevos con número de jugadores (`#PLAYERS`), y deshacer para propiedades, charts nuevos y borrados.
- ✅ Guardado sin pérdidas: conserva tags desconocidos (`#QUESTMODS`, `#HEARTS`…) y el texto exacto de los charts sin editar.
- ✅ Verificador de biblioteca: `--check <carpeta>` carga, guarda y vuelve a leer cada simfile.
- ✅ Opciones de línea de comandos: `--chart`, `--beat`, `--view`, `--select`, `--mirror`, `--tempo`, `--minimap`, `--noteskin`.
- ✅ Fuentes del sistema para títulos en coreano, japonés y chino.
- ✅ Panel con conteo de pasos, saltos, holds, rolls, minas y fakes, y avisos de lectura (colas sin hold, etc.).

## 12. Ideas para añadir

Propuestas iniciales (se pueden quitar o reordenar):

- [ ] Interpretar las notas especiales de StepP1 (`h`, `p`, `Y`, `v`, `w`…) con icono y descripción.
- [ ] Editar los atributos StepF2 de cada nota (fake, hidden, sudden, vanish, jugador).
- [ ] Poner, mover y estirar notas con el mouse.
- [ ] Co-op: mostrar sólo un jugador, reasignar el jugador de una selección y avisar si dos jugadores chocan.
- [ ] Importar / exportar formato UCS (Andamiro).
- [ ] Convertir entre estilos pump: single ↔ double ↔ half-double.
- [ ] Validador de charts con lista navegable (holds sin cola, notas en warps, saltos imposibles…).
- [ ] Estadísticas: NPS, gráfico de densidad, stream breakdown estilo pump.
- [ ] Comparar dos charts (diff visual).
- [ ] Modo práctica: jugar el chart con el teclado, con juicios, o autoplay.
- [ ] Repetir una región en bucle para afinar la sincronía.
- [ ] Autoguardado y recuperación tras un cierre inesperado.
- [ ] Plantillas de patrones pump (drills, runs, brackets, twists).
- [ ] Navegador de la biblioteca de canciones con búsqueda.
- [ ] Vista previa del video de fondo (`#PREVIEWVID` / BGCHANGES).
- [ ] Interfaz en español e inglés y colores configurables.

### Ideas del usuario

- [x] Minimapa a la derecha con todo el chart, recuadro de la vista y navegación con click.
- [x] Minimapa que se desplaza con canciones largas (como ArrowVortex) y más legible en singles.
- [x] Zoom independiente del minimapa en modo notas y densidad.
- [x] El cursor no baja del compás 0 al navegar.
- [x] Click en el fondo no mueve el chart (sólo el click sobre las columnas).
- [x] Holds que relucen al ritmo del BPM.
- [x] Usar los noteskins de XSanity.
- [x] No distribuir noteskins de terceros: botón para importarlos del simulador que tenga el usuario.
- [x] Nombre del proyecto: Step Charter.
- [x] Los holds terminan visualmente en el snap donde se colocan (remate centrado en la fila final).
- [x] Gimmicks visibles en el chart: vista de juego con speeds y scrolls, y zonas de warps, fakes, stops y delays.

_(Se irán agregando aquí a medida que las menciones.)_
