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

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use thiserror::Error;

use crate::parser::{EmpireDesign, ParseError, has_same_identity, parse_empire_designs};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ConflictPolicy {
    #[default]
    Skip,
    Replace,
    Append,
}

impl ConflictPolicy {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Skip => "Skip duplicates",
            Self::Replace => "Replace duplicates",
            Self::Append => "Append anyway",
        }
    }
}

#[derive(Debug, Default)]
pub struct ImportReport {
    pub imported: usize,
    pub replaced: usize,
    pub skipped: usize,
    pub backup_path: Option<PathBuf>,
}

#[derive(Debug, Error)]
pub enum FileError {
    #[error("failed to read '{path}': {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to write '{path}': {source}")]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to copy '{from}' to '{to}': {source}")]
    Copy {
        from: PathBuf,
        to: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to remove temporary file '{path}': {source}")]
    RemoveTemp {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to parse Stellaris empire file: {0}")]
    Parse(#[from] ParseError),
    #[error("no empires were selected")]
    NoSelection,
    #[error("the selected file has no usable filename")]
    MissingFileName,
}

pub fn load_empire_file(path: &Path) -> Result<(String, Vec<EmpireDesign>), FileError> {
    let content = fs::read_to_string(path).map_err(|source| FileError::Read {
        path: path.to_owned(),
        source,
    })?;
    let empires = parse_empire_designs(&content)?;

    Ok((content, empires))
}

pub fn export_selected_bundle(
    empires: &[EmpireDesign],
    selected: &BTreeSet<usize>,
    output_path: &Path,
) -> Result<usize, FileError> {
    if selected.is_empty() {
        return Err(FileError::NoSelection);
    }

    let selected_empires = selected
        .iter()
        .filter_map(|index| empires.get(*index).cloned())
        .collect::<Vec<_>>();
    let count = selected_empires.len();

    if count == 0 {
        return Err(FileError::NoSelection);
    }

    let output = format_empire_bundle(&selected_empires, "\r\n");
    parse_empire_designs(&output)?;

    fs::write(output_path, output).map_err(|source| FileError::Write {
        path: output_path.to_owned(),
        source,
    })?;

    Ok(count)
}

pub fn import_selected_to_file(
    target_path: &Path,
    source_empires: &[EmpireDesign],
    selected: &BTreeSet<usize>,
    policy: ConflictPolicy,
) -> Result<ImportReport, FileError> {
    if selected.is_empty() {
        return Err(FileError::NoSelection);
    }

    let (target_content, target_empires) = load_empire_file(target_path)?;
    let selected_empires = selected
        .iter()
        .filter_map(|index| source_empires.get(*index).cloned())
        .collect::<Vec<_>>();

    if selected_empires.is_empty() {
        return Err(FileError::NoSelection);
    }

    let (new_content, mut report) =
        apply_import(&target_content, &target_empires, &selected_empires, policy);

    if report.imported == 0 && report.replaced == 0 {
        return Ok(report);
    }

    let backup_path = backup_path_for(target_path)?;
    fs::copy(target_path, &backup_path).map_err(|source| FileError::Copy {
        from: target_path.to_owned(),
        to: backup_path.clone(),
        source,
    })?;

    let temp_path = temp_path_for(target_path)?;
    parse_empire_designs(&new_content)?;
    fs::write(&temp_path, new_content).map_err(|source| FileError::Write {
        path: temp_path.clone(),
        source,
    })?;
    fs::copy(&temp_path, target_path).map_err(|source| FileError::Copy {
        from: temp_path.clone(),
        to: target_path.to_owned(),
        source,
    })?;
    fs::remove_file(&temp_path).map_err(|source| FileError::RemoveTemp {
        path: temp_path,
        source,
    })?;

    report.backup_path = Some(backup_path);
    Ok(report)
}

pub fn apply_import(
    target_content: &str,
    target_empires: &[EmpireDesign],
    incoming: &[EmpireDesign],
    policy: ConflictPolicy,
) -> (String, ImportReport) {
    let newline = detect_newline(target_content);
    let has_bom = target_content.starts_with('\u{feff}');
    let mut report = ImportReport::default();
    let mut output_empires = target_empires.to_vec();
    let mut replaced_indices = BTreeSet::<usize>::new();

    for empire in incoming {
        let conflict_index = output_empires
            .iter()
            .position(|target| has_same_identity(target, empire));

        match (conflict_index, policy) {
            (Some(_), ConflictPolicy::Skip) => report.skipped += 1,
            (Some(index), ConflictPolicy::Replace) => {
                if replaced_indices.insert(index) {
                    output_empires[index] = empire.clone();
                    report.replaced += 1;
                } else {
                    report.skipped += 1;
                }
            }
            _ => {
                output_empires.push(empire.clone());
                report.imported += 1;
            }
        }
    }

    let output = format_empire_file(&output_empires, newline, has_bom);

    (output, report)
}

pub fn format_empire_bundle(empires: &[EmpireDesign], newline: &str) -> String {
    format_empire_file(empires, newline, false)
}

fn format_empire_file(empires: &[EmpireDesign], newline: &str, include_bom: bool) -> String {
    let mut output = String::new();

    if include_bom {
        output.push('\u{feff}');
    }

    for (index, empire) in empires.iter().enumerate() {
        if index > 0 {
            output.push_str(newline);
        }
        output.push_str(&normalize_empire_block(&empire.raw_text, newline));
        output.push_str(newline);
    }

    output
}

fn normalize_empire_block(text: &str, newline: &str) -> String {
    normalize_line_endings(text.trim()).replace('\n', newline)
}

fn normalize_line_endings(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

fn detect_newline(text: &str) -> &'static str {
    if text.contains("\r\n") { "\r\n" } else { "\n" }
}

fn backup_path_for(path: &Path) -> Result<PathBuf, FileError> {
    suffixed_path(path, "civshare", "bak")
}

fn temp_path_for(path: &Path) -> Result<PathBuf, FileError> {
    suffixed_path(path, "civshare", "tmp")
}

fn suffixed_path(path: &Path, label: &str, extension: &str) -> Result<PathBuf, FileError> {
    let file_name = path.file_name().ok_or(FileError::MissingFileName)?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    let process_id = std::process::id();
    let file_name = format!(
        "{}.{}.{}.{}.{}",
        file_name.to_string_lossy(),
        label,
        stamp,
        process_id,
        extension
    );

    Ok(path.with_file_name(file_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = include_str!("../reference/user_empire_designs_v3.4.txt");

    #[test]
    fn skip_policy_leaves_duplicate_out() {
        let target = parse_empire_designs(SAMPLE).expect("target should parse");
        let incoming = vec![target[0].clone()];

        let (_output, report) = apply_import(SAMPLE, &target, &incoming, ConflictPolicy::Skip);

        assert_eq!(report.skipped, 1);
        assert_eq!(report.imported, 0);
        assert_eq!(report.replaced, 0);
    }

    #[test]
    fn append_policy_adds_duplicate() {
        let target = parse_empire_designs(SAMPLE).expect("target should parse");
        let incoming = vec![target[0].clone()];

        let (output, report) = apply_import(SAMPLE, &target, &incoming, ConflictPolicy::Append);

        assert_eq!(report.imported, 1);
        assert!(output.len() > SAMPLE.len());
    }

    #[test]
    fn import_output_is_canonical_and_parseable() {
        let crlf_sample = SAMPLE.replace('\n', "\r\n");
        let target = parse_empire_designs(&crlf_sample).expect("target should parse");
        let incoming = vec![target[0].clone()];

        let (output, report) =
            apply_import(&crlf_sample, &target, &incoming, ConflictPolicy::Append);

        assert_eq!(report.imported, 1);
        assert!(output.contains("}\r\n\r\n\""));
        assert!(!output.replace("\r\n", "").contains('\n'));
        parse_empire_designs(&output).expect("canonical output should parse");
    }

    #[test]
    fn formatted_bundle_has_clean_block_boundaries() {
        let target = parse_empire_designs(SAMPLE).expect("target should parse");
        let output = format_empire_bundle(&target[..2], "\r\n");

        assert!(output.contains("}\r\n\r\n\"Treasured Caretakers\"="));
        assert!(output.ends_with("\r\n"));
        parse_empire_designs(&output).expect("formatted bundle should parse");
    }

    #[test]
    fn replace_policy_replaces_duplicate() {
        let target = parse_empire_designs(SAMPLE).expect("target should parse");
        let mut incoming_empire = target[0].clone();
        incoming_empire.raw_text = incoming_empire
            .raw_text
            .replace("auth_dictatorial", "auth_test_replaced");

        let (output, report) =
            apply_import(SAMPLE, &target, &[incoming_empire], ConflictPolicy::Replace);

        assert_eq!(report.replaced, 1);
        assert!(output.contains("auth_test_replaced"));
    }
}
