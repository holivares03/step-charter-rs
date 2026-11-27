# Step Charter

Editor de charts inspirado en ArrowVortex, escrito en Rust y enfocado en **Pump It Up**
(archivos `.ssc` de StepMania 5 / StepF2 / XSanity).

## Estado actual

- **Lectura de `.ssc` y `.sm`**, incluido el split timing (timing propio por chart), que usan
  5706 de los 7113 charts de la biblioteca de prueba.
- **Estilos**: `pump-single` (5), `pump-halfdouble` (6), `pump-double` (10), `pump-couple` y
  `pump-routine` (10) y co-op de StepF2 `pump-single-p` / `pump-double-p` con `#PLAYERS`.
  Los charts dance también cargan (con un noteskin vectorial).
- **Notas de StepF2/StepP1**: tokens `{110}`, `{2m8}`, `{2|n|1|0}` (fake), jugador por nota,
  bloques por jugador con `&` (`#SEPARATENOTES:YES`). Los tipos especiales que todavía no
  interpretamos se dibujan con su letra y se guardan sin cambios.
- **Timing**: BPMs, stops, delays y warps con el mismo orden de procesamiento que
  StepMania 5. Se dibujan también time signatures, tickcounts, combos, speeds, scrolls,
  fakes y labels.
- **Vista**: por tiempo (como ArrowVortex) o por filas, zoom, tamaño de notas, reverse,
  waveform, fondo de la canción y compases según las time signatures.
- **Minimapa** a la derecha (Shift+M), en modo notas o densidad: escala fija que se desplaza
  con el chart (como en ArrowVortex), recuadro de la parte visible, click o arrastre para
  navegar, rueda para avanzar de compás en compás y Ctrl+rueda para el zoom (cada modo guarda el suyo).
- **Noteskins** (*Vista → Noteskin*): la app trae el noteskin pump de ArrowVortex y uno
  vectorial. Los noteskins de tu simulador se importan con *Importar noteskins de un
  simulador…*: Step Charter busca XSanity, StepF2/StepP1, StepMania, ITGmania u OutFox en
  tus unidades (o eliges la carpeta a mano) y copia sus noteskins pump a la carpeta de datos
  de la app (`%APPDATA%\Step Charter\data\noteskins\pump`). Sin importar nada, también se
  leen los noteskins del simulador de la canción abierta (la carpeta `NoteSkins` junto a
  `Songs`). En co-op, cada jugador usa el noteskin de `#PRELOADNOTESKIN`. Notas, holds y
  receptores se animan con el beat.
- **Audio**: mp3/ogg/wav/flac, reproducción sincronizada, velocidad, volumen y ticks de beat y
  de notas.
- **Edición**: poner y quitar notas con 1..0 (si mantienes la tecla y mueves el cursor se
  crea un hold; con Shift, un roll), minas con Alt, jugador activo en co-op (`/`),
  deshacer/rehacer, selección por región (Tab, mouse, Ctrl+A) y por tipo de nota
  (pasos, saltos, minas, holds, rolls, fakes, cuantización, jugador), copiar/cortar/pegar,
  borrar y espejo horizontal/vertical/completo.
- **Canción y charts**: simfile nuevo desde un audio, nuevo chart (Shift+N), borrar chart
  (Ctrl+Supr), propiedades del chart (Shift+C) y de la canción (Shift+P): textos, archivos,
  Display BPM, preview y los demás tags.
- **Editor de tempo** (Shift+T): offset (±1/±10 ms), BPM inicial, mover el primer beat
  (±½ / ±1 beat), y BPM, stops, delays, warps y scrolls en el cursor,
  lista de todos los segmentos, y crear o descartar el timing propio de un chart.
- **Guardado seguro**: se conservan los tags desconocidos (`#QUESTMODS`, `#HEARTS`, …) y
  el texto exacto de los charts que no editaste. La primera vez que se sobrescribe un
  archivo se crea un `.bak` con el original.

### Extras que no tiene ArrowVortex

- **Vista de juego** (Shift+G): vista por filas con `#SCROLLS` y `#SPEEDS` aplicados como en
  StepMania 5, así los charts con gimmicks se ven como en el juego. Al reproducir, los stops
  congelan la vista y los warps saltan. La barra de estado muestra el speed y el scroll actuales.
- **Zonas de gimmicks**: warps (morado), fakes (rayado), stops y delays (rojo / naranja, con su
  duración en la vista por tiempo), transiciones de speed (franja azul a la derecha) y scrolls
  (franja a la izquierda: cian > 1, verde < 1, amarillo = 0, magenta = hacia atrás).
- **Co-op**: cada nota lleva un color y una insignia con el número de su jugador.
- **Importar noteskins** desde el simulador instalado, en vez de distribuirlos con la app.
- **Verificador de biblioteca**: `step-charter --check <carpeta>` carga todos los simfiles,
  prueba guardar y volver a leer cada uno e informa de cualquier diferencia.

## Compilar y ejecutar

```
cargo build --release
target\release\step-charter.exe [archivo.ssc | audio.ogg] [--chart N] [--beat B] [--view time|rows|game]
    [--select A:B] [--mirror h|v|hv] [--tempo] [--minimap notas|densidad] [--noteskin nombre]
    [--dialog cancion|chart|nuevo|importar]
```

También puedes abrir archivos con Ctrl+O, desde *Archivo → Recientes* o arrastrándolos a la
ventana.

## Atajos (F1 dentro del programa)

| Tecla | Acción |
|---|---|
| Ctrl+O / Ctrl+S / Ctrl+Shift+S | Abrir / Guardar / Guardar como |
| Espacio | Reproducir / pausa |
| ↑ ↓ / rueda | Mover el cursor según el snap |
| ← → | Cambiar el snap |
| Re Pág / Av Pág (Shift: beat) | Compás anterior / siguiente |
| Inicio / Fin | Inicio / final del chart |
| F5 / F6 | Chart anterior / siguiente |
| + / − , Ctrl+rueda | Zoom |
| Ctrl + / Ctrl − | Tamaño de las notas |
| 1..9, 0 | Nota en la columna (mantener + mover = hold) |
| Shift+1..0 / Alt+1..0 | Roll / mina |
| / | Jugador activo (co-op) |
| Ctrl+Z / Ctrl+Y | Deshacer / rehacer |
| Tab / arrastrar / Shift+click | Seleccionar región |
| Ctrl+A / Esc | Seleccionar todo / quitar selección |
| Ctrl+C / Ctrl+X / Ctrl+V / Supr | Copiar / cortar / pegar / borrar |
| Y / T / G | Espejo horizontal / vertical / completo |
| Shift+T | Editor de tempo y sincronía |
| Shift+P / Shift+C | Propiedades de la canción / del chart |
| Shift+N / Ctrl+Supr | Nuevo chart / borrar chart |
| F3 / F4 | Tick de beat / tick de notas |
| Shift+↑↓ / Shift+←→ | Volumen / velocidad |
| Shift+V / Shift+G / Shift+R | Vista por tiempo o filas / vista de juego (SCROLLS + SPEEDS) / reverse |
| Shift+M | Mostrar / ocultar el minimapa |

## Estructura

- `src/simfile/`: modelo y formatos (`msd.rs` tokenizador, `ssc.rs`/`sm.rs` carga y escritura,
  `notes.rs`, `timing.rs` conversión beat↔tiempo, `style.rs` estilos).
- `src/audio.rs`: decodificación (symphonia) y salida (cpal).
- `src/notefield.rs` y `src/minimap.rs`: dibujo del notefield y del minimapa.
- `src/noteskin.rs`: noteskin integrado, carga de noteskins de StepMania/XSanity, búsqueda de
  simuladores e importación.
- `src/edit.rs`: selección, portapapeles, espejo y edición de segmentos de timing.
- `src/app.rs`: interfaz (egui/eframe), entrada y edición; los diálogos están en
  `src/app/dialogs.rs` y `src/app/skins_import.rs`.
- `src/check.rs`: verificador de biblioteca.
- `assets/noteskins/pump/`: texturas del noteskin "pump" de ArrowVortex 1.0.1, incrustadas en
  el ejecutable.

Si pones una carpeta `noteskins/pump` junto al ejecutable (instalación portátil), también se
leen sus noteskins. Esa carpeta no se sube al repositorio.

## Limitaciones conocidas

- La resolución es de 48 filas por beat, igual que StepMania y ArrowVortex. Las medidas con
  un número de líneas que no divide 192 se redondean (en la biblioteca de prueba hay 1 entre
  468 591).

## Créditos

- Inspirado en [ArrowVortex](https://github.com/kade-github/arrowvortex). Las texturas del
  noteskin pump integrado vienen de ArrowVortex 1.0.1.
- Los noteskins de XSanity, StepF2, StepMania y demás simuladores pertenecen a sus autores y
  no se distribuyen con Step Charter: cada usuario los importa desde su propia instalación.

## Lista de funciones

La comparación completa con ArrowVortex (qué está hecho, parcial o pendiente), los extras
propios y las ideas por añadir están en [FUNCIONES.md](FUNCIONES.md).

## Licencia

Step Charter es software libre bajo la [GNU General Public License v3.0](LICENSE) o, a tu
elección, cualquier versión posterior (la misma licencia que ArrowVortex).
