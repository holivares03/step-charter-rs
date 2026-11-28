# Cambios

Todos los cambios importantes de Step Charter. El formato sigue
[Keep a Changelog](https://keepachangelog.com/es-ES/1.1.0/) y las versiones siguen
[Semantic Versioning](https://semver.org/lang/es/).

## [Sin publicar]

### Agregado
- Editor de tempo: sección *StepMania 5* para editar en el cursor compás, ticks, combo,
  speed (factor, duración y unidad), scroll, fakes y labels.
- Ctrl+B: cambio de BPM límite con el mismo valor (en el cursor o en los extremos de la
  selección), para ajustar el tempo sólo en esa zona sin alterar la sincronía.
- *Tempo → Convertir la selección en stop / en stutter* (el stutter no desfasa lo que sigue).
- *Tempo → Copiar tempo de la selección / Pegar tempo en el cursor*.

### Cambiado
- En el editor de tempo, poner en 0 un stop, delay, warp o fake (o vaciar un label) quita el
  segmento en vez de dejarlo con valor 0.

### Arreglado
- Arrastrando en el minimapa hasta el tope, el cursor no llegaba al beat 0 (quedaba uno o dos
  beats más abajo).
- El cursor se podía seguir bajando en el chart después del final de la canción. Ahora llega
  hasta la última nota, el último cambio de tempo o el final de la música (lo más lejano),
  más un margen hasta el compás siguiente, como en ArrowVortex. El minimapa termina en el
  mismo lugar y la reproducción se detiene ahí.

## [0.2.0] - 2026-11-27

Edición de notas: convertir tipos, escalar, cambiar de lado, cambiar de jugador e insertar o
borrar beats.

### Agregado
- *Editar → Convertir*: notas ↔ minas, fakes y lifts, holds → pasos o minas y holds ↔ rolls
  (tecla º, bajo Esc). Los tokens de StepF2 se actualizan; de fake a nota se conserva el hold.
- *Editar → Escalar*: 2:1, 3:2 y 4:3 (expandir) y 1:2, 2:3 y 3:4 (comprimir), respecto a la
  primera nota de la selección.
- *Editar → Cambiar de lado*: pasa las notas a la otra mitad del pad sin espejarlas (en double,
  del pad de P1 al de P2).
- En co-op, `/` con notas seleccionadas las pasa al jugador siguiente.
- Editor de tempo: insertar o borrar beats en el cursor, en este chart o en todos, moviendo
  notas y cambios de tempo.

### Arreglado
- El `.exe` trae el ícono de Step Charter (antes el Explorador mostraba el genérico de
  Windows) y el Administrador de tareas lo muestra como "Step Charter".

## [0.1.0] - 2026-11-27

Primera versión pública.

### Archivos y charts
- Abrir `.ssc` y `.sm` (incluido el split timing) y guardar `.ssc` sin pérdidas: se conservan
  los tags desconocidos y el texto de los charts sin editar; `.bak` del original la primera vez.
- Simfile nuevo desde un audio, nuevo chart, borrar chart, propiedades del chart y de la
  canción (textos, archivos, Display BPM, preview y demás tags), archivos recientes y aviso de
  cambios sin guardar.
- Estilos pump single, half-double, double, couple y routine, co-op de StepF2
  (`pump-single-p` / `pump-double-p` con `#PLAYERS`) y estilos dance.
- Notas de StepF2/StepP1 (`{110}`, `{2m8}`, `{2|n|1|0}`) y bloques por jugador.

### Vista
- Vista por tiempo, por filas y vista de juego con `#SCROLLS` y `#SPEEDS` como en StepMania 5.
- Zonas de gimmicks (warps, fakes, stops, delays, speeds y scrolls), minimapa de notas o
  densidad, waveform, fondo de la canción, reverse y receptores animados con el beat.
- Noteskin pump de ArrowVortex integrado, uno vectorial e importación de los noteskins del
  simulador instalado (XSanity, StepF2/P1, StepMania, ITGmania, OutFox).

### Edición
- Poner y quitar notas, holds, rolls y minas con el teclado; deshacer y rehacer.
- Selección por región y por tipo de nota, cuantización o jugador; copiar, cortar, pegar,
  borrar y espejos horizontal, vertical y completo.
- Editor de tempo: offset, BPM inicial, BPM, stops, delays, warps y scrolls, lista de
  segmentos y timing propio por chart.

### Audio y herramientas
- Reproducción sincronizada de mp3, ogg, wav y flac con volumen, velocidad y ticks.
- Verificador de biblioteca (`--check`) y opciones de línea de comandos.

[Sin publicar]: https://github.com/holivares03/step-charter-rs/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/holivares03/step-charter-rs/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/holivares03/step-charter-rs/releases/tag/v0.1.0
