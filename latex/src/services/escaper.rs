//! LaTeX special-character escaping helpers.
//!
//! LaTeX treats the following characters specially and they must be escaped
//! before being emitted into a `.tex` file:
//!
//!   &  %  $  #  _  {  }  ~  ^  \
//!
//! Backslash must become `\textbackslash{}`, not `\\`, because `\\` is a
//! line-break in LaTeX.  Tilde and caret also need the command form.

//! Escape a plain-text string so it is safe to embed in a LaTeX document body.
pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str(r"\textbackslash{}"),
            '&'  => out.push_str(r"\&"),
            '%'  => out.push_str(r"\%"),
            '$'  => out.push_str(r"\$"),
            '#'  => out.push_str(r"\#"),
            '_'  => out.push_str(r"\_"),
            '{'  => out.push_str(r"\{"),
            '}'  => out.push_str(r"\}"),
            '~'  => out.push_str(r"\textasciitilde{}"),
            '^'  => out.push_str(r"\textasciicircum{}"),
            '’'  => out.push('\''),
            '‘'  => out.push('`'),
            '“'  => out.push_str("``"),
            '”'  => out.push_str("''"),
            '—'  => out.push_str("---"),
            '–'  => out.push_str("--"),
            // ── Unicode symbols → LaTeX commands ─────────────────────────────
            '✓' | '✔' => out.push_str(r"\checkmark"),
            '✗' | '✘' | '✕' => out.push_str(r"$\times$"),
            '•'        => out.push_str(r"\textbullet{}"),
            '▪' | '▸' | '►' => out.push_str(r"$\blacksquare$"),
            '→'        => out.push_str(r"$\rightarrow$"),
            '←'        => out.push_str(r"$\leftarrow$"),
            '↔'        => out.push_str(r"$\leftrightarrow$"),
            '⇒'        => out.push_str(r"$\Rightarrow$"),
            '≤'        => out.push_str(r"$\leq$"),
            '≥'        => out.push_str(r"$\geq$"),
            '≠'        => out.push_str(r"$\neq$"),
            '±'        => out.push_str(r"$\pm$"),
            '×'        => out.push_str(r"$\times$"),
            '÷'        => out.push_str(r"$\div$"),
            '°'        => out.push_str(r"\textdegree{}"),
            '©'        => out.push_str(r"\textcopyright{}"),
            '®'        => out.push_str(r"\textregistered{}"),
            '™'        => out.push_str(r"\texttrademark{}"),
            '…'        => out.push_str(r"\ldots{}"),
            '€'        => out.push_str(r"\texteuro{}"),
            '£'        => out.push_str(r"\pounds{}"),
            '§'        => out.push_str(r"\S{}"),
            '¶'        => out.push_str(r"\P{}"),
            other => out.push(other),
        }
    }
    out
}

/// Escape a string intended for use inside a LaTeX command argument (e.g. a
/// URL inside `\href{}`).  Less aggressive — only escapes characters that
/// would break the argument delimiter.
pub fn escape_url(s: &str) -> String {
    // For URLs we only need to worry about `%` (comment in LaTeX) and `#` (fragment).
    s.replace('%', "\\%").replace('#', "\\#")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_escapes() {
        assert_eq!(escape("100% done"), r"100\% done");
        assert_eq!(escape("$price"), r"\$price");
        assert_eq!(escape("a & b"), r"a \& b");
        assert_eq!(escape("x_i^2"), r"x\_i\textasciicircum{}2");
        assert_eq!(escape(r"C:\path"), r"C:\textbackslash{}path");
    }

    #[test]
    fn empty_string() {
        assert_eq!(escape(""), "");
    }

    #[test]
    fn no_special_chars() {
        let s = "Hello, world!";
        assert_eq!(escape(s), s);
    }
}
