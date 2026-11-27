# Cómo trabajamos en Step Charter

## Ramas

- `main` siempre compila y pasa las pruebas. Nada a medio hacer entra a `main`.
- Cada cambio va en su propia rama, con un prefijo que dice qué es:
  `feat/transformar-notas`, `fix/cursor-deshacer`, `docs/readme`, `refactor/…`, `chore/…`.

## Commits

- Mensajes en español con el prefijo de [Conventional Commits](https://www.conventionalcommits.org/es/v1.0.0/):

  | Prefijo | Para |
  |---|---|
  | `feat:` | una función nueva |
  | `fix:` | un arreglo |
  | `docs:` | documentación |
  | `refactor:` | reorganizar código sin cambiar lo que hace |
  | `test:` | pruebas |
  | `chore:` | dependencias, configuración, CI |

  Ejemplos: `feat: notas → minas, fakes y lifts`, `fix: el cursor salta al cambio deshecho`.
- Un cambio lógico por commit. Dentro de una rama los commits pueden ser pequeños.

## Pull requests

- Todo entra a `main` por pull request, aunque sea un cambio propio.
- El título del PR lleva el prefijo (`feat: …`): al fusionar con **Squash and merge** se
  convierte en el único commit de ese cambio en `main`.
- La verificación automática (GitHub Actions) debe pasar antes de fusionar.
- Antes de pedir la fusión:
  - `cargo test` pasa.
  - Si el cambio toca la lectura o el guardado de simfiles, `step-charter --check <carpeta>`
    no muestra cambios nuevos.
  - `FUNCIONES.md` refleja el cambio (estados ✅ / 🟡 / ❌ y la línea de resumen).
  - `CHANGELOG.md` tiene la entrada en **Sin publicar**.
  - `README.md` está al día si cambian atajos, opciones o el uso.

## Versiones y releases

Las versiones siguen [Semantic Versioning](https://semver.org/lang/es/). Mientras el
proyecto esté en 0.x: la versión menor (0.**2**.0) sube con funciones nuevas y la de
parche (0.2.**1**) con arreglos.

Para publicar una versión:

1. En un PR (`chore: versión X.Y.Z`): subir `version` en `Cargo.toml` y en `CHANGELOG.md`
   mover lo de **Sin publicar** a una sección `## [X.Y.Z] - fecha`, con su enlace al final.
2. Fusionar el PR.
3. Crear y subir el tag desde `main` actualizado:

   ```
   git switch main && git pull
   git tag vX.Y.Z
   git push origin vX.Y.Z
   ```

4. GitHub Actions comprueba que el tag coincide con `Cargo.toml`, corre las pruebas, compila
   el `.exe` y publica la Release con el `.zip` y las notas de esa versión del CHANGELOG.

## Bugs e ideas

- Los bugs y las ideas de otras personas van en
  [Issues](https://github.com/holivares03/step-charter-rs/issues).
- La lista maestra de funciones (paridad con ArrowVortex, extras e ideas) es
  [FUNCIONES.md](FUNCIONES.md).
