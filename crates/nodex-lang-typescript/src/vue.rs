//! Vue single-file components (`.vue`): the `<script>` and `<script setup>` blocks are lowered
//! as TypeScript/JavaScript.
//!
//! The parser reads a copy of the file with the same byte length where everything outside the
//! script blocks (template, styles, the tags themselves) is blanked; line breaks stay, so spans
//! and line numbers point into the real `.vue` file.

/// The script of a component, ready for the parser.
pub struct Script {
    /// The file with everything but the script contents blanked.
    pub text: String,
    /// `lang="ts"` (TypeScript grammar) or not (TSX grammar, which also reads JavaScript).
    pub typescript: bool,
    /// The component has `<script setup>`: its top level runs once per component instance.
    pub setup: bool,
}

pub fn script(text: &str) -> Script {
    let bytes = text.as_bytes();
    let mut keep = vec![false; bytes.len()];
    let lower = text.to_ascii_lowercase();
    let mut typescript = false;
    let mut setup = false;
    let mut from = 0;
    while let Some(open) = lower[from..].find("<script").map(|i| i + from) {
        let Some(tag_end) = lower[open..].find('>').map(|i| i + open) else { break };
        let attrs = &lower[open + 7..tag_end];
        let Some(close) = lower[tag_end..].find("</script").map(|i| i + tag_end) else { break };
        if attrs.contains("lang=\"ts\"") || attrs.contains("lang='ts'") {
            typescript = true;
        }
        if attrs.split_whitespace().any(|a| a == "setup" || a.starts_with("setup")) {
            setup = true;
        }
        for k in &mut keep[tag_end + 1..close] {
            *k = true;
        }
        from = close + 8;
    }
    let out: Vec<u8> = bytes
        .iter()
        .zip(&keep)
        .map(|(&b, &k)| if k || b == b'\n' || b == b'\r' { b } else { b' ' })
        .collect();
    Script { text: String::from_utf8(out).unwrap_or_default(), typescript, setup }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_scripts_and_blanks_the_rest() {
        let sfc = "<template>\n  <p>{{ n }} é</p>\n</template>\n<script setup lang=\"ts\">\nconst n = 1;\n</script>\n<style>p{}</style>\n";
        let s = script(sfc);
        assert_eq!(s.text.len(), sfc.len());
        assert!(s.typescript && s.setup);
        assert!(s.text.contains("const n = 1;"));
        assert!(!s.text.contains("template") && !s.text.contains("style") && !s.text.contains("script"));
        assert_eq!(s.text.matches('\n').count(), sfc.matches('\n').count());
    }
}
