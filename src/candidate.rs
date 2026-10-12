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
    let parts: Vec<_> = text.split("```").collect();
    if parts.len() != 3 {
        bail!("expected exactly one complete fenced Python block");
    }
    let block = parts[1];
    let Some(code) = block
        .strip_prefix("python\n")
        .or_else(|| block.strip_prefix("python\r\n"))
    else {
        bail!("candidate fence must identify Python");
    };
    if code.trim().is_empty() || code.len() > 64 * 1024 {
        bail!("candidate is empty or exceeds 64 KiB");
    }
    Ok(format!("{}\n", code.trim_end()))
}

#[cfg(test)]
mod tests {
    use super::*;

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
