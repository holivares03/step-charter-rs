# Cambios

Todos los cambios importantes de Step Charter. El formato sigue
[Keep a Changelog](https://keepachangelog.com/es-ES/1.1.0/) y las versiones siguen
[Semantic Versioning](https://semver.org/lang/es/).

## [Sin publicar]

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

[Sin publicar]: https://github.com/holivares03/step-charter-rs/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/holivares03/step-charter-rs/releases/tag/v0.1.0
