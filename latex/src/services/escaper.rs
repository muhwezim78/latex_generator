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
            // Use \ensuremath{} rather than $...$ so these are safe both in
            // normal text AND inside an already-open math environment.
            '✓' | '✔' => out.push_str(r"\checkmark"),
            '✗' | '✘' | '✕' => out.push_str(r"\ensuremath{\times}"),
            '•'        => out.push_str(r"\textbullet{}"),
            '▪' | '▸' | '►' => out.push_str(r"\ensuremath{\blacksquare}"),
            '→'        => out.push_str(r"\ensuremath{\rightarrow}"),
            '←'        => out.push_str(r"\ensuremath{\leftarrow}"),
            '↔'        => out.push_str(r"\ensuremath{\leftrightarrow}"),
            '⇒'        => out.push_str(r"\ensuremath{\Rightarrow}"),
            '⇔'        => out.push_str(r"\ensuremath{\Leftrightarrow}"),
            '≤'        => out.push_str(r"\ensuremath{\leq}"),
            '≥'        => out.push_str(r"\ensuremath{\geq}"),
            '≠'        => out.push_str(r"\ensuremath{\neq}"),
            '≈'        => out.push_str(r"\ensuremath{\approx}"),
            '∝'        => out.push_str(r"\ensuremath{\propto}"),
            '±'        => out.push_str(r"\ensuremath{\pm}"),
            '∓'        => out.push_str(r"\ensuremath{\mp}"),
            '−'        => out.push_str(r"\ensuremath{-}"),
            '×'        => out.push_str(r"\ensuremath{\times}"),
            '÷'        => out.push_str(r"\ensuremath{\div}"),
            '∞'        => out.push_str(r"\ensuremath{\infty}"),
            '∈'        => out.push_str(r"\ensuremath{\in}"),
            '∉'        => out.push_str(r"\ensuremath{\notin}"),
            '⊂'        => out.push_str(r"\ensuremath{\subset}"),
            '⊆'        => out.push_str(r"\ensuremath{\subseteq}"),
            '∪'        => out.push_str(r"\ensuremath{\cup}"),
            '∩'        => out.push_str(r"\ensuremath{\cap}"),
            '∑'        => out.push_str(r"\ensuremath{\sum}"),
            '∏'        => out.push_str(r"\ensuremath{\prod}"),
            '∫'        => out.push_str(r"\ensuremath{\int}"),
            '√'        => out.push_str(r"\ensuremath{\sqrt{}}"),
            '∂'        => out.push_str(r"\ensuremath{\partial}"),
            '∇'        => out.push_str(r"\ensuremath{\nabla}"),
            '∆'        => out.push_str(r"\ensuremath{\Delta}"),
            '∴'        => out.push_str(r"\ensuremath{\therefore}"),
            '∵'        => out.push_str(r"\ensuremath{\because}"),
            '∥'        => out.push_str(r"\ensuremath{\parallel}"),
            '⊥'        => out.push_str(r"\ensuremath{\perp}"),
            '∠'        => out.push_str(r"\ensuremath{\angle}"),
            '∀'        => out.push_str(r"\ensuremath{\forall}"),
            '∃'        => out.push_str(r"\ensuremath{\exists}"),
            '¬'        => out.push_str(r"\ensuremath{\neg}"),
            '∧'        => out.push_str(r"\ensuremath{\wedge}"),
            '∨'        => out.push_str(r"\ensuremath{\vee}"),
            // ── Greek letters (lowercase) ─────────────────────────────────────
            'α' => out.push_str(r"\ensuremath{\alpha}"),
            'β' => out.push_str(r"\ensuremath{\beta}"),
            'γ' => out.push_str(r"\ensuremath{\gamma}"),
            'δ' => out.push_str(r"\ensuremath{\delta}"),
            'ε' => out.push_str(r"\ensuremath{\varepsilon}"),
            'ζ' => out.push_str(r"\ensuremath{\zeta}"),
            'η' => out.push_str(r"\ensuremath{\eta}"),
            'θ' => out.push_str(r"\ensuremath{\theta}"),
            'ι' => out.push_str(r"\ensuremath{\iota}"),
            'κ' => out.push_str(r"\ensuremath{\kappa}"),
            'λ' => out.push_str(r"\ensuremath{\lambda}"),
            'μ' => out.push_str(r"\ensuremath{\mu}"),
            'ν' => out.push_str(r"\ensuremath{\nu}"),
            'ξ' => out.push_str(r"\ensuremath{\xi}"),
            'π' => out.push_str(r"\ensuremath{\pi}"),
            'ρ' => out.push_str(r"\ensuremath{\rho}"),
            'σ' => out.push_str(r"\ensuremath{\sigma}"),
            'τ' => out.push_str(r"\ensuremath{\tau}"),
            'υ' => out.push_str(r"\ensuremath{\upsilon}"),
            'φ' => out.push_str(r"\ensuremath{\varphi}"),
            'χ' => out.push_str(r"\ensuremath{\chi}"),
            'ψ' => out.push_str(r"\ensuremath{\psi}"),
            'ω' => out.push_str(r"\ensuremath{\omega}"),
            // ── Greek letters (uppercase) ─────────────────────────────────────
            'Γ' => out.push_str(r"\ensuremath{\Gamma}"),
            'Δ' => out.push_str(r"\ensuremath{\Delta}"),
            'Θ' => out.push_str(r"\ensuremath{\Theta}"),
            'Λ' => out.push_str(r"\ensuremath{\Lambda}"),
            'Ξ' => out.push_str(r"\ensuremath{\Xi}"),
            'Π' => out.push_str(r"\ensuremath{\Pi}"),
            'Σ' => out.push_str(r"\ensuremath{\Sigma}"),
            'Υ' => out.push_str(r"\ensuremath{\Upsilon}"),
            'Φ' => out.push_str(r"\ensuremath{\Phi}"),
            'Χ' => out.push_str(r"\ensuremath{\Chi}"),
            'Ψ' => out.push_str(r"\ensuremath{\Psi}"),
            'Ω' => out.push_str(r"\ensuremath{\Omega}"),
            // ── Invisible Unicode formatting characters → drop silently ───────
            '\u{2061}' => {} // U+2061 FUNCTION APPLICATION — no visual representation
            '\u{200B}' => {} // U+200B ZERO WIDTH SPACE
            '\u{FEFF}' => {} // U+FEFF BOM / ZERO WIDTH NO-BREAK SPACE
            // ── Typographic punctuation ───────────────────────────────────────
            '°'        => out.push_str(r"\textdegree{}"),
            '©'        => out.push_str(r"\textcopyright{}"),
            '®'        => out.push_str(r"\textregistered{}"),
            '™'        => out.push_str(r"\texttrademark{}"),
            '…'        => out.push_str(r"\ldots{}"),
            '€'        => out.push_str(r"\texteuro{}"),
            '£'        => out.push_str(r"\pounds{}"),
            '§'        => out.push_str(r"\S{}"),
            '¶'        => out.push_str(r"\P{}"),
            // Ignore Microsoft Word PUA characters (often used for bullets/checkboxes)
            '\u{f8f1}' | '\u{f8f2}' | '\u{f8f3}' => {}
            other => out.push(other),
        }
    }
    out
}

/// Escape a string intended for use inside a `\href{}` URL argument.
///
/// Only the characters that TeX itself would misinterpret are escaped:
/// - `%` → `\%`  (comment character — would swallow the rest of the URL)
/// - `#` → `\#`  (macro argument separator in some contexts)
///
/// `&` is intentionally left unescaped: the `hyperref` package's `\href{}`
/// command processes its first argument as a URL verbatim, so `&` is safe and
/// must not be altered (percent-encoding it would produce a broken link).
pub fn escape_url(s: &str) -> String {
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
    fn url_escapes() {
        assert_eq!(escape_url("http://example.com/path"), "http://example.com/path");
        // & passes through unchanged — hyperref handles it natively in \href{}
        assert_eq!(escape_url("foo?a=1&b=2"), "foo?a=1&b=2");
        assert_eq!(escape_url("path#section"), "path\\#section");
        // % in an already-encoded URL is escaped to \% so TeX doesn't treat it as a comment
        assert_eq!(escape_url("already%20encoded"), "already\\%20encoded");
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
