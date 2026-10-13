use anyhow::{Result, bail};

/// Only complete fenced candidates are executable. Reasoning is never code.
pub fn extract(text: &str) -> Result<String> {
    let text = text.trim_start();
    let text = if let Some(reasoning) = text.strip_prefix("<think>") {
        reasoning
            .split_once("</think>")
            .ok_or_else(|| anyhow::anyhow!("reasoning did not close"))?
            .1
    } else {
        text
    };
    let mut opening = None;
    let mut closing = None;
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        if let Some((width, info)) = markdown_fence(line) {
            match opening {
                None => {
                    if info != "python" {
                        bail!("candidate fence must identify Python");
                    }
                    opening = Some((width, offset + line.len()));
                }
                Some((minimum, _)) if closing.is_none() => {
                    if width >= minimum && info.is_empty() {
                        closing = Some(offset);
                    }
                }
                Some(_) => bail!("expected exactly one complete fenced Python block"),
            }
        }
        offset += line.len();
    }
    let (Some((_, start)), Some(end)) = (opening, closing) else {
        bail!("expected exactly one complete fenced Python block");
    };
    let code = &text[start..end];
    if code.trim().is_empty() || code.len() > 64 * 1024 {
        bail!("candidate is empty or exceeds 64 KiB");
    }
    Ok(format!("{}\n", code.trim_end()))
}

// Closing Markdown fences may have at most three leading spaces. More deeply
// indented backticks, such as Python docstring examples, are ordinary code.
fn markdown_fence(line: &str) -> Option<(usize, &str)> {
    let line = line.trim_end_matches(['\n', '\r']);
    let indent = line.bytes().take_while(|b| *b == b' ').count();
    if indent > 3 {
        return None;
    }
    let content = &line[indent..];
    let width = content.bytes().take_while(|b| *b == b'`').count();
    if width < 3 {
        return None;
    }
    Some((width, content[width..].trim()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_backticks_allow_literal_examples_and_reject_ambiguity() {
        let code =
            "def f():\n    \"\"\"\n    ```pycon\n    >>> f()\n    ```\n    \"\"\"\n    return 1\n";
        assert_eq!(extract(&format!("````python\n{code}````\n")).unwrap(), code);
        assert!(extract(&format!("````python\n{code}")).is_err());
        assert!(extract("````python\nx=1\n````\n```python\ny=2\n```").is_err());
    }
    #[test]
    fn ordinary_three_tick_fence_preserves_indented_docstring_examples() {
        let code =
            include_str!("../tasks/repository/naturalsize-rollover/repo/humanize/filesize.py");
        assert_eq!(extract(&format!("```python\n{code}```\n")).unwrap(), code);
        assert!(extract("```python\nx=1\n```\n```python\ny=2\n```").is_err());
    }
    #[test]
    fn accepts_one_complete_python_block() {
        assert_eq!(
            extract("Reasoning\n```python\nx = 1\n```\n").unwrap(),
            "x = 1\n"
        );
    }

    #[test]
    fn only_final_answer_code_is_executable() {
        assert_eq!(
            extract("<think>draft\n```python\nwrong()\n```\n</think>\n```python\nright()\n```\n")
                .unwrap(),
            "right()\n"
        );
        assert!(extract("<think>```python\nwrong()\n```").is_err());
        assert!(extract("<think>```python\nwrong()\n```</think>no final code").is_err());
    }

    #[test]
    fn rejects_ambiguous_incomplete_or_empty_candidates() {
        for text in [
            "x = 1",
            "```python\nx = 1",
            "```python\n```",
            "```rust\nx\n```",
            "```python\nx\n```\n```python\ny\n```",
        ] {
            assert!(extract(text).is_err(), "{text}");
        }
    }
}
