//! Tokenizador del formato MSD (`#TAG:param:param;`) usado por .sm y .ssc.
//!
//! Replica el comportamiento de `MsdFile` de StepMania:
//! - `//` inicia un comentario hasta el final de la línea (en cualquier lugar).
//! - `\` escapa el siguiente carácter.
//! - `:` separa parámetros, `;` termina el valor.
//! - Un `#` al inicio de una línea (ignorando espacios) termina un valor al que
//!   le faltaba el `;`, para tolerar archivos mal formados.

#[derive(Debug, Clone, PartialEq)]
pub struct MsdValue {
    /// Nombre del tag, tal como aparece (sin `#`).
    pub tag: String,
    /// Parámetros después del tag (separados por `:`), ya sin escapes.
    pub params: Vec<String>,
    /// Texto fuente exacto del valor (desde el primer `:` hasta el `;`), con
    /// comentarios y escapes intactos. Permite reescribir sin cambios lo que no se tocó.
    pub raw: String,
}

impl MsdValue {
    /// Nombre del tag en mayúsculas.
    pub fn key(&self) -> String {
        self.tag.trim().to_ascii_uppercase()
    }

    #[cfg(test)]
    pub fn first(&self) -> &str {
        self.params.first().map(|s| s.as_str()).unwrap_or("")
    }
}

pub fn parse(text: &str) -> Vec<MsdValue> {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let mut out: Vec<MsdValue> = Vec::new();

    let mut reading = false;
    let mut current: Vec<String> = Vec::new();
    let mut buf = String::new();
    // Índice donde empieza el valor (tras el primer ':').
    let mut raw_start: Option<usize> = None;

    let finish = |current: &mut Vec<String>, buf: &mut String, out: &mut Vec<MsdValue>, raw: String| {
        current.push(std::mem::take(buf));
        let mut params = std::mem::take(current);
        let tag = if params.is_empty() { String::new() } else { params.remove(0) };
        out.push(MsdValue { tag, params, raw });
    };
    let slice = |a: Option<usize>, b: usize| -> String {
        match a {
            Some(a) if a <= b => chars[a..b].iter().collect(),
            _ => String::new(),
        }
    };

    let mut i = 0;
    while i < n {
        let c = chars[i];

        // Comentarios
        if c == '/' && i + 1 < n && chars[i + 1] == '/' {
            while i < n && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }

        if reading && c == '#' {
            // ¿Es el primer carácter no-espacio de la línea?
            let mut j = i;
            let mut first_in_line = true;
            while j > 0 {
                let p = chars[j - 1];
                if p == '\n' || p == '\r' {
                    break;
                }
                if !p.is_whitespace() {
                    first_in_line = false;
                    break;
                }
                j -= 1;
            }
            if first_in_line {
                // Termina el valor anterior (al que le faltó el ';').
                let trimmed = buf.trim_end().to_string();
                buf = trimmed;
                let raw = slice(raw_start, j).trim_end().to_string();
                finish(&mut current, &mut buf, &mut out, raw);
                reading = false;
            }
        }

        if !reading {
            if c == '#' {
                reading = true;
                current.clear();
                buf.clear();
                raw_start = None;
            }
            i += 1;
            continue;
        }

        match c {
            ':' => {
                if raw_start.is_none() {
                    raw_start = Some(i + 1);
                }
                current.push(std::mem::take(&mut buf));
            }
            ';' => {
                let raw = slice(raw_start, i);
                finish(&mut current, &mut buf, &mut out, raw);
                reading = false;
            }
            '\\' if i + 1 < n => {
                buf.push(chars[i + 1]);
                i += 1;
            }
            _ => buf.push(c),
        }
        i += 1;
    }

    if reading {
        let raw = slice(raw_start, n);
        finish(&mut current, &mut buf, &mut out, raw);
    }

    out
}

/// `//` iniciaría un comentario: hay que escapar cada `/` que toca a otro.
fn is_double_slash(chars: &[char], i: usize) -> bool {
    (i + 1 < chars.len() && chars[i + 1] == '/') || (i > 0 && chars[i - 1] == '/')
}

/// Escapa un texto para escribirlo como un único parámetro MSD.
pub fn escape(s: &str) -> String {
    escape_value(s)
}

/// Escapa un valor que puede contener varios parámetros (se unen con `:` sin escapar).
pub fn escape_params(params: &[String]) -> String {
    params.iter().map(|p| escape_value(p)).collect::<Vec<_>>().join(":")
}

/// Escapa `;`, `:`, `\`, los `//` (comentarios) y un `#` al inicio de línea.
pub fn escape_value(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut at_line_start = true;
    for (i, &c) in chars.iter().enumerate() {
        match c {
            ';' | '\\' | ':' => {
                out.push('\\');
                out.push(c);
                at_line_start = false;
            }
            '/' if is_double_slash(&chars, i) => {
                out.push('\\');
                out.push(c);
                at_line_start = false;
            }
            '#' if at_line_start => {
                out.push('\\');
                out.push(c);
                at_line_start = false;
            }
            '\n' => {
                out.push(c);
                at_line_start = true;
            }
            c if c.is_whitespace() => out.push(c),
            _ => {
                out.push(c);
                at_line_start = false;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_tags() {
        let v = parse("#TITLE:Hola;\n#BPMS:0.000=120.000;\n");
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].key(), "TITLE");
        assert_eq!(v[0].first(), "Hola");
        assert_eq!(v[1].first(), "0.000=120.000");
    }

    #[test]
    fn comments_and_two_tags_one_line() {
        let v = parse("// comentario\n#PREVIEW:intro.mp3;#PREVIEWVID:503_P.mpg;\n");
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].first(), "intro.mp3");
        assert_eq!(v[1].key(), "PREVIEWVID");
    }

    #[test]
    fn missing_semicolon() {
        let v = parse("#TITLE:Hola\n#ARTIST:Yo;");
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].first(), "Hola");
        assert_eq!(v[1].first(), "Yo");
    }

    #[test]
    fn escapes_and_params() {
        let v = parse("#TITLE:Re\\:Start;#NOTES:a:b:c;");
        assert_eq!(v[0].first(), "Re:Start");
        assert_eq!(v[1].params, vec!["a", "b", "c"]);
        assert_eq!(v[1].raw, "a:b:c");
        let v = parse("#NOTES:\n// measure 0\n1000\n;");
        assert_eq!(v[0].raw, "\n// measure 0\n1000\n");
        assert_eq!(v[0].first(), "\n\n1000\n");
        let v = parse(&format!("#ARTIST:{};", escape("A // B >///<")));
        assert_eq!(v[0].first(), "A // B >///<");
        assert_eq!(escape("Re:Start"), "Re\\:Start");
    }
}
