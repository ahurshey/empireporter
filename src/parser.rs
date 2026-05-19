// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 Alex Hurshman
//
// This file is part of CivShare.
//
// CivShare is free software: you can redistribute it and/or modify it under the
// terms of the GNU General Public License as published by the Free Software
// Foundation, version 3 only.
//
// CivShare is distributed in the hope that it will be useful, but WITHOUT ANY
// WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR
// A PARTICULAR PURPOSE. See the GNU General Public License for more details.

use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmpireDesign {
    pub name: String,
    pub key: Option<String>,
    pub start_byte: usize,
    pub end_byte: usize,
    pub raw_text: String,
    pub summary: EmpireSummary,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EmpireSummary {
    pub authority: Option<String>,
    pub government: Option<String>,
    pub origin: Option<String>,
    pub species_class: Option<String>,
    pub portrait: Option<String>,
    pub civics: Vec<String>,
    pub ethics: Vec<String>,
}

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("expected top-level empire name at byte {0}")]
    ExpectedEmpireName(usize),
    #[error("unterminated quoted string starting at byte {0}")]
    UnterminatedString(usize),
    #[error("expected '=' after empire name '{name}' at byte {byte}")]
    ExpectedEquals { name: String, byte: usize },
    #[error("expected '{{' after empire name '{name}' at byte {byte}")]
    ExpectedOpenBrace { name: String, byte: usize },
    #[error("unterminated empire block '{name}' starting at byte {byte}")]
    UnterminatedBlock { name: String, byte: usize },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Section {
    Species,
    Civics,
}

pub fn parse_empire_designs(input: &str) -> Result<Vec<EmpireDesign>, ParseError> {
    let mut empires = Vec::new();
    let mut pos = skip_ws_and_bom(input, 0);

    while pos < input.len() {
        if input.as_bytes().get(pos) != Some(&b'"') {
            return Err(ParseError::ExpectedEmpireName(pos));
        }

        let start_byte = pos;
        let (name, after_name) = parse_quoted(input, pos)?;
        pos = skip_ascii_ws(input, after_name);

        if input.as_bytes().get(pos) != Some(&b'=') {
            return Err(ParseError::ExpectedEquals { name, byte: pos });
        }

        pos = skip_ascii_ws(input, pos + 1);
        if input.as_bytes().get(pos) != Some(&b'{') {
            return Err(ParseError::ExpectedOpenBrace { name, byte: pos });
        }

        let close_byte =
            find_matching_brace(input, pos).ok_or_else(|| ParseError::UnterminatedBlock {
                name: name.clone(),
                byte: start_byte,
            })?;
        let end_byte = include_single_trailing_newline(input, close_byte + 1);
        let raw_text = input[start_byte..end_byte].to_owned();
        let summary = extract_summary(&raw_text);
        let key = summary_key(&raw_text);

        empires.push(EmpireDesign {
            name,
            key,
            start_byte,
            end_byte,
            raw_text,
            summary,
        });

        pos = skip_ascii_ws(input, end_byte);
    }

    Ok(empires)
}

pub fn has_same_identity(left: &EmpireDesign, right: &EmpireDesign) -> bool {
    left.name == right.name
        || left
            .key
            .as_ref()
            .zip(right.key.as_ref())
            .is_some_and(|(left_key, right_key)| left_key == right_key)
}

#[cfg(test)]
pub fn validate_single_empire(input: &str) -> Result<EmpireDesign, String> {
    let empires = parse_empire_designs(input).map_err(|err| err.to_string())?;
    match empires.as_slice() {
        [empire] => Ok(empire.clone()),
        [] => Err("No empire design blocks were found.".to_owned()),
        _ => Err("Expected exactly one empire design block, but found multiple.".to_owned()),
    }
}

fn summary_key(raw: &str) -> Option<String> {
    let mut depth = 0usize;

    for line in raw.lines() {
        let trimmed = line.trim();
        if depth == 1 {
            if let Some(value) = assignment_quoted_value(trimmed, "key") {
                return Some(value);
            }
        }
        depth = apply_brace_delta(line, depth);
    }

    None
}

fn extract_summary(raw: &str) -> EmpireSummary {
    let mut summary = EmpireSummary::default();
    let mut depth = 0usize;
    let mut pending_section: Option<Section> = None;
    let mut active_section: Option<(Section, usize)> = None;

    for line in raw.lines() {
        let trimmed = line.trim();
        let line_depth = depth;

        if line_depth == 1 {
            if summary.authority.is_none() {
                summary.authority = assignment_quoted_value(trimmed, "authority");
            }
            if summary.government.is_none() {
                summary.government = assignment_quoted_value(trimmed, "government");
            }
            if summary.origin.is_none() {
                summary.origin = assignment_quoted_value(trimmed, "origin");
            }
            if let Some(ethic) = assignment_quoted_value(trimmed, "ethic") {
                summary.ethics.push(ethic);
            }
            if is_block_assignment(trimmed, "species") {
                pending_section = Some(Section::Species);
            }
            if is_block_assignment(trimmed, "civics") {
                pending_section = Some(Section::Civics);
            }
        }

        if let Some((section, section_depth)) = active_section {
            match section {
                Section::Species if line_depth == section_depth => {
                    if summary.species_class.is_none() {
                        summary.species_class = assignment_quoted_value(trimmed, "class");
                    }
                    if summary.portrait.is_none() {
                        summary.portrait = assignment_quoted_value(trimmed, "portrait");
                    }
                }
                Section::Civics if line_depth == section_depth => {
                    if let Some(civic) = first_quoted(trimmed) {
                        summary.civics.push(civic);
                    }
                }
                _ => {}
            }
        }

        let next_depth = apply_brace_delta(line, depth);
        if let Some(section) = pending_section {
            if next_depth > line_depth {
                active_section = Some((section, next_depth));
                pending_section = None;
            }
        }
        if active_section.is_some_and(|(_, section_depth)| next_depth < section_depth) {
            active_section = None;
        }
        depth = next_depth;
    }

    summary
}

fn skip_ws_and_bom(input: &str, mut pos: usize) -> usize {
    if input.as_bytes().starts_with(&[0xEF, 0xBB, 0xBF]) {
        pos = pos.max(3);
    }
    skip_ascii_ws(input, pos)
}

fn skip_ascii_ws(input: &str, mut pos: usize) -> usize {
    while let Some(byte) = input.as_bytes().get(pos) {
        if matches!(byte, b' ' | b'\t' | b'\r' | b'\n') {
            pos += 1;
        } else {
            break;
        }
    }
    pos
}

fn parse_quoted(input: &str, start: usize) -> Result<(String, usize), ParseError> {
    let bytes = input.as_bytes();
    let mut pos = start + 1;
    let mut output = String::new();
    let mut chunk_start = pos;
    let mut escaped = false;

    while pos < bytes.len() {
        let byte = bytes[pos];
        if escaped {
            output.push_str(&input[chunk_start..pos - 1]);
            output.push(byte as char);
            escaped = false;
            pos += 1;
            chunk_start = pos;
            continue;
        }

        match byte {
            b'\\' => {
                escaped = true;
                pos += 1;
            }
            b'"' => {
                output.push_str(&input[chunk_start..pos]);
                return Ok((output, pos + 1));
            }
            _ => pos += 1,
        }
    }

    Err(ParseError::UnterminatedString(start))
}

fn find_matching_brace(input: &str, open_pos: usize) -> Option<usize> {
    let bytes = input.as_bytes();
    let mut pos = open_pos;
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;

    while pos < bytes.len() {
        let byte = bytes[pos];

        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            pos += 1;
            continue;
        }

        match byte {
            b'"' => in_string = true,
            b'{' => depth += 1,
            b'}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(pos);
                }
            }
            _ => {}
        }

        pos += 1;
    }

    None
}

fn include_single_trailing_newline(input: &str, pos: usize) -> usize {
    if input[pos..].starts_with("\r\n") {
        pos + 2
    } else if input[pos..].starts_with('\n') {
        pos + 1
    } else {
        pos
    }
}

fn assignment_quoted_value(line: &str, key: &str) -> Option<String> {
    let value = assignment_value(line, key)?;
    first_quoted(value)
}

fn assignment_value<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let rest = line.strip_prefix(key)?;
    let rest = rest.trim_start();
    let rest = rest.strip_prefix('=')?;
    Some(rest.trim_start())
}

fn is_block_assignment(line: &str, key: &str) -> bool {
    assignment_value(line, key).is_some()
}

fn first_quoted(line: &str) -> Option<String> {
    let start = line.find('"')?;
    parse_quoted(line, start).ok().map(|(value, _)| value)
}

fn apply_brace_delta(line: &str, mut depth: usize) -> usize {
    let mut in_string = false;
    let mut escaped = false;

    for byte in line.bytes() {
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            continue;
        }

        match byte {
            b'"' => in_string = true,
            b'{' => depth += 1,
            b'}' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }

    depth
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = include_str!("../reference/user_empire_designs_v3.4.txt");

    #[test]
    fn parses_reference_empires() {
        let empires = parse_empire_designs(SAMPLE).expect("reference sample should parse");

        assert_eq!(empires.len(), 8);
        assert_eq!(empires[0].name, "Miidarian Authority");
        assert_eq!(
            empires[0].summary.authority.as_deref(),
            Some("auth_dictatorial")
        );
        assert_eq!(empires[0].summary.species_class.as_deref(), Some("AVI"));
        assert_eq!(empires[7].name, "Edacious Maginon Masticators");
        assert_eq!(
            empires[7].summary.origin.as_deref(),
            Some("origin_necrophage")
        );
    }

    #[test]
    fn validates_one_exported_block() {
        let empires = parse_empire_designs(SAMPLE).expect("reference sample should parse");
        let single =
            validate_single_empire(&empires[3].raw_text).expect("single block should parse");

        assert_eq!(single.name, "Animated Whispers");
        assert_eq!(
            single.summary.government.as_deref(),
            Some("gov_machine_industrial")
        );
    }
}
