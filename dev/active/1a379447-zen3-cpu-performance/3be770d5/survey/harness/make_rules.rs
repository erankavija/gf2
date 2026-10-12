//! Make rules as GNU make reads them, for the dependency files of `-MD`.

/// Prerequisites of every rule in `rules`. A backslash-newline continues a
/// rule, an odd run of backslashes quotes the space or `#` after it, an even
/// run halves, and `$$` is a dollar sign.
pub fn prerequisites(rules: &str) -> Vec<String> {
    let mut found = Vec::new();
    for rule in rules.replace("\\\n", " ").lines() {
        let Some(listed) = rule
            .match_indices(':')
            .map(|(colon, _)| &rule[colon + 1..])
            .find(|rest| rest.chars().next().is_none_or(char::is_whitespace))
        else {
            continue;
        };
        let mut word = String::new();
        let mut backslashes = 0;
        let mut characters = listed.chars().peekable();
        while let Some(character) = characters.next() {
            if character == '\\' {
                backslashes += 1;
                continue;
            }
            let quotable = character == ' ' || character == '#';
            let kept = if quotable {
                backslashes / 2
            } else {
                backslashes
            };
            word.extend(std::iter::repeat_n('\\', kept));
            let quoted = quotable && backslashes % 2 == 1;
            backslashes = 0;
            if character == '$' && characters.peek() == Some(&'$') {
                characters.next();
            }
            if character.is_whitespace() && !quoted {
                if !word.is_empty() {
                    found.push(std::mem::take(&mut word));
                }
            } else {
                word.push(character);
            }
        }
        word.extend(std::iter::repeat_n('\\', backslashes));
        if !word.is_empty() {
            found.push(word);
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::prerequisites;

    #[test]
    fn a_path_with_a_space_is_one_prerequisite() {
        let rules = "out/unit.o: unit.cpp /opt/aff3ct\\ tree/decoder.hpp \\\n \
                     /opt/cost$$/a\\#b.hpp back\\\\ slash.hpp\n\n\
                     second\\ unit.o: C:/drive.hpp\nphony.hpp:\n";
        assert_eq!(
            prerequisites(rules),
            [
                "unit.cpp",
                "/opt/aff3ct tree/decoder.hpp",
                "/opt/cost$/a#b.hpp",
                "back\\",
                "slash.hpp",
                "C:/drive.hpp",
            ]
        );
    }
}
