// SPDX-License-Identifier: GPL-3.0-or-later
//! Audit one save or every immediate child end.sc4 of a golden corpus directory.
use fc4_formats::city_save::{CitySave, Report};
use fc4_formats::Package;
use std::path::{Path, PathBuf};

fn saves(path: &Path) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    if path.is_file() {
        return Ok(vec![path.to_path_buf()]);
    }
    let mut files = Vec::new();
    for child in std::fs::read_dir(path)? {
        let save = child?.path().join("end.sc4");
        if save.is_file() {
            files.push(save);
        }
    }
    files.sort();
    Ok(files)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let path = args
        .next()
        .ok_or("usage: fc4-save-roundtrip <save.sc4|golden-directory> [--report file]")?;
    let report_path = match args.next() {
        Some(flag) if flag == "--report" => Some(args.next().ok_or("missing report path")?),
        Some(_) => return Err("unknown option".into()),
        None => None,
    };
    if args.next().is_some() {
        return Err("unexpected argument".into());
    }
    let files = saves(Path::new(&path))?;
    if files.is_empty() {
        return Err("no saves found".into());
    }
    eprintln!("Auditing {} saves", files.len());
    let started = std::time::Instant::now();
    let mut report = Report::default();
    let mut failed = 0;
    for (i, path) in files.iter().enumerate() {
        let result = (|| {
            let package = Package::open(path).map_err(|e| e.to_string())?;
            let model = CitySave::read(&package)?;
            model.audit(&model.write()?)
        })();
        match result {
            Ok(r) => {
                for difference in &r.differences {
                    eprintln!("{}: {difference}", path.display());
                }
                report.merge(r);
            }
            Err(error) => {
                failed += 1;
                eprintln!("{}: ERROR {error}", path.display());
            }
        }
        if (i + 1) % 25 == 0 {
            eprintln!(
                "{} / {} saves ({:.1}s)",
                i + 1,
                files.len(),
                started.elapsed().as_secs_f64()
            );
        }
    }
    let summary = format!(
        "{} saves, {failed} failed ({:.1}s)\n{}",
        files.len(),
        started.elapsed().as_secs_f64(),
        report.table()
    );
    if let Some(path) = report_path {
        std::fs::write(path, &summary)?;
    }
    println!("{summary}");
    if failed != 0 || !report.differences.is_empty() {
        return Err("round-trip failures".into());
    }
    Ok(())
}
