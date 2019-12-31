use std::path::{Path, PathBuf};
use std::ffi::OsStr;
use std::env;

use target::{arch, os, os_family, endian, pointer_width};
use dirs;
use chrono;
use regex::Regex;
use unicode_width::UnicodeWidthStr;

use crate::var::{VarValue};

// arch: aarch64, arm, asmjs, hexagon, mips, mips64, msp430, powerpc, powerpc64, s390x
//       sparc, sparc64, wasm32, x86, x86_64, xcore
// os: android, bitrig, dragonfly, emscripten, freebsd, haiku, ios, linux, macos,
//     netbsd, openbsd, solaris, windows
// os_family: unix, windows
// pointer_width: 32, 64
// endian: big, little
//
// file'n'dir functions
//   test: is_file, is_dir, exists
//   parts: stem, ext, dir, base
//   change: with_ext, with_stem, with_filename, add_ext
//   user: home, config_dir, doc_dir, desktop_dir, temp
//   misc: path_join
//   util: print
//   time: format

type FuncResult = Result<VarValue, String>;
enum CheckType {
    IsFile,
    IsDir,
    Exists,
}
enum PathPart {
    Stem,
    Name,
    Dir,
    Ext,
}
enum SysPath {
    Temp,
    Home,
    Docs,
    Config,
}
enum Where {
    All,
    Left,
    Right,
}
enum StrCase {
    Up,
    Low,
}

pub(crate) fn run_func(name: &str, args: &[VarValue]) -> FuncResult {
    let lowstr = name.to_lowercase();
    match lowstr.as_str() {
        "os" => Ok(VarValue::from(os())),
        "family" => Ok(VarValue::from(os_family())),
        "bit" => Ok(VarValue::from(pointer_width())),
        "arch" => Ok(VarValue::from(arch())),
        "endian" => Ok(VarValue::from(endian())),
        "is_file" | "is-file" | "isfile" => all_are(args, CheckType::IsFile),
        "is_dir" | "is-dir" | "isdir" => all_are(args, CheckType::IsDir),
        "exists" => all_are(args, CheckType::Exists),
        "stem" => extract_part(args, PathPart::Stem),
        "ext" => extract_part(args, PathPart::Ext),
        "dir" => extract_part(args, PathPart::Dir),
        "filename" => extract_part(args, PathPart::Name),
        "add_ext" | "add-ext" => add_ext(args),
        "with_ext" | "with-ext" => replace_ext(args),
        "with_filename" | "with-filename"
            | "with_name" | "with-name" => replace_name(args),
        "with_stem" | "with-stem" => replace_stem(args),
        "join" => join_path(args),
        "temp" | "temp_dir" | "temp-dir" => system_path(SysPath::Temp),
        "home" | "home_dir" | "home-dir"
            | "user_dir" | "user-dir" => system_path(SysPath::Home),
        "config" | "config_dir" | "config-dir" => system_path(SysPath::Config),
        "documents" | "docs_dir" | "docs-dir" => system_path(SysPath::Docs),
        "print" => print_all(args, false),
        "println" => print_all(args, true),
        "time" | "format-time" | "format_time"
            | "time-format" | "time_format" => format_time(args),
        "trim" => trim_string(args, Where::All),
        "trim_left" | "trim-left" | "trim_start" | "trim-start" => trim_string(args, Where::Left),
        "trim_right" | "trim-right" | "trim_end" | "trim-end" => trim_string(args, Where::Right),
        "starts-with" | "starts_with" => starts_with(args),
        "ends-with" | "ends_with" => ends_with(args),
        "lowcase" => change_case(args, StrCase::Low),
        "upcase" => change_case(args, StrCase::Up),
        "contains" => contains(args),
        "replace" => replace(args),
        "match" => match_regex(args),
        "pad-center" | "pad_center" => pad(args, Where::All),
        "pad-left" | "pad_left" => pad(args, Where::Left),
        "pad-right" | "pad_right" => pad(args, Where::Right),
        _ => Err(format!("function {} not found", name)),
    }
}

fn all_are(args: &[VarValue], tp: CheckType) -> FuncResult {
    if args.is_empty() {
        return Ok(VarValue::Int(0));
    }
    for arg in args {
        let s = arg.to_string();
        let p = Path::new(&s);
        let ok = match tp {
            CheckType::IsFile => p.is_file(),
            CheckType::IsDir => p.is_dir(),
            CheckType::Exists => p.exists(),
        };
        if !ok {
            return Ok(VarValue::Int(0));
        }
    }
    Ok(VarValue::Int(1))
}

fn extract_part(args: &[VarValue], tp: PathPart) -> FuncResult {
    if args.is_empty() {
        return Ok(VarValue::Int(0));
    }
    let s = args[0].to_string();
    let p = Path::new(&s);
    let empty = OsStr::new("");
    let empty_path = Path::new("");
    match tp {
        PathPart::Stem => Ok(VarValue::from(p.file_stem().unwrap_or(&empty).to_string_lossy().to_string())),
        PathPart::Ext => Ok(VarValue::from(p.extension().unwrap_or(&empty).to_string_lossy().to_string())),
        PathPart::Dir => Ok(VarValue::from(p.parent().unwrap_or(&empty_path).to_string_lossy().to_string())),
        PathPart::Name => Ok(VarValue::from(p.file_name().unwrap_or(&empty).to_string_lossy().to_string())),
    }
}

fn replace_ext(args: &[VarValue]) -> FuncResult {
    if args.is_empty() {
        return Err("path undefined".to_string());
    }
    if args.len() == 1 {
        return Ok(args[0].clone());
    }
    let mut p = Path::new(&args[0].to_string()).to_owned();
    let ext = if args.len() == 1 {
        String::new()
    } else {
        args[1].to_string()
    };
    p.set_extension(ext);
    Ok(VarValue::Str(p.to_string_lossy().to_string()))
}

fn add_ext(args: &[VarValue]) -> FuncResult {
    if args.is_empty() {
        return Err("path undefined".to_string());
    }
    if args.len() == 1 {
        return Ok(args[0].clone());
    }
    let p = args[0].to_string();
    let mut e = args[1].to_string();
    if e.is_empty() {
        return Ok(VarValue::Str(p));
    }
    if !e.starts_with('.') {
        e = format!(".{}", e);
    }
    Ok(VarValue::Str(p+&e))
}

fn replace_name(args: &[VarValue]) -> FuncResult {
    if args.is_empty() {
        return Err("path undefined".to_string());
    }
    if args.len() == 1 {
        return Err("new name undefined".to_string());
    }
    let mut p = Path::new(&args[0].to_string()).to_owned();
    let new_name = Path::new(&args[1].to_string()).to_owned();
    p.set_file_name(new_name);
    Ok(VarValue::Str(p.to_string_lossy().to_string()))
}

fn replace_stem(args: &[VarValue]) -> FuncResult {
    if args.is_empty() {
        return Err("path undefined".to_string());
    }
    if args.len() == 1 {
        return Err("new stem undefined".to_string());
    }
    let arg_str = args[0].to_string();
    let p = Path::new(&arg_str);
    let new_stem = args[1].to_string();
    if new_stem.is_empty() {
        return Err("new stem undefined".to_string());
    }
    let empty = OsStr::new("");
    let empty_path = Path::new("");
    let ext = p.extension().unwrap_or(&empty).to_string_lossy().to_string();
    let dir = p.parent().unwrap_or(&empty_path);
    let fname = if ext.is_empty() {
        new_stem
    } else {
        new_stem + "." + &ext
    };
    Ok(VarValue::Str(dir.join(fname).to_string_lossy().to_string()))
}

fn join_path(args: &[VarValue]) -> FuncResult {
    if args.is_empty() {
        return Ok(VarValue::Str(String::new()));
    }
    if args.len() == 1 {
        return Ok(args[0].clone());
    }
    let mut path = PathBuf::from(args[0].to_string());
    for a in &args[1..] {
        let astr = a.to_string();
        let p = Path::new(&astr);
        path = path.join(p);
    }
    Ok(VarValue::Str(path.to_string_lossy().to_string()))
}

fn system_path(pathtype: SysPath) -> FuncResult {
    match pathtype {
        SysPath::Temp => Ok(VarValue::Str(env::temp_dir().to_string_lossy().to_string())),
        SysPath::Home => match dirs::home_dir() {
            None => Err("user home directory undefined".to_string()),
            Some(p) => Ok(VarValue::Str(p.to_string_lossy().to_string())),
        },
        SysPath::Config => match dirs::config_dir() {
            None => Err("user configuration directory indefined".to_string()),
            Some(p) => Ok(VarValue::Str(p.to_string_lossy().to_string())),
        },
        SysPath::Docs => match dirs::document_dir() {
            None => Err("user document directory indefined".to_string()),
            Some(p) => Ok(VarValue::Str(p.to_string_lossy().to_string())),
        },
    }
}

fn print_all(args: &[VarValue], add_new_line: bool) -> FuncResult {
    for (idx, v) in args.iter().enumerate() {
        if idx != 0 {
            print!(" ");
        }
        print!("{}", v.to_string());
    }
    if add_new_line {
        println!();
    }
    Ok(VarValue::Int(1))
}

fn format_time(args: &[VarValue]) -> FuncResult {
    let now = chrono::Local::now();
    let format = if args.is_empty() {
        "%Y%m%d-%H%M%S".to_string()
    } else {
        args[0].to_flat_string()
    };
    let r = match format.to_lowercase().as_str() {
        "2822" | "rfc2822" => now.to_rfc2822(),
        "3339" | "rfc3339" => now.to_rfc3339(),
        _ => now.format(&format).to_string(),
    };
    Ok(VarValue::Str(r))
}

fn trim_string(args: &[VarValue], dir: Where) -> FuncResult {
    if args.is_empty() {
        return Ok(VarValue::Str(String::new()));
    }

    let s = args[0].to_string();
    if args.len() == 1 {
        let st = match dir {
            Where::All => s.trim(),
            Where::Left => s.trim_start(),
            Where::Right => s.trim_end(),
        };
        return Ok(VarValue::from(st));
    }

    let what = args[1].to_string().chars().next();
    let c = match what {
        None => return Ok(VarValue::Str(s)),
        Some(cc) => cc,
    };
    let st = match dir {
        Where::All => s.trim_matches(c),
        Where::Left => s.trim_start_matches(c),
        Where::Right => s.trim_end_matches(c),
    };
    Ok(VarValue::from(st))
}

fn starts_with(args: &[VarValue]) -> FuncResult {
    if args.len() < 2 {
        return Ok(VarValue::Int(1));
    }

    let s = args[0].to_string();
    for a in args[1..].iter() {
        let what = a.to_string();
        if s.starts_with(&what) {
            return Ok(VarValue::Int(1));
        }
    }
    Ok(VarValue::Int(0))
}

fn ends_with(args: &[VarValue]) -> FuncResult {
    if args.len() < 2 {
        return Ok(VarValue::Int(1));
    }

    let s = args[0].to_string();
    for a in args[1..].iter() {
        let what = a.to_string();
        if s.ends_with(&what) {
            return Ok(VarValue::Int(1));
        }
    }
    Ok(VarValue::Int(0))
}

fn change_case(args: &[VarValue], case: StrCase) -> FuncResult {
    if args.is_empty() {
        return Ok(VarValue::Str(String::new()));
    }

    let s = args[0].to_string();
    let res = match case {
        StrCase::Up => s.to_uppercase(),
        StrCase::Low => s.to_lowercase(),
    };
    Ok(VarValue::Str(res))
}

fn contains(args: &[VarValue]) -> FuncResult {
    if args.len() < 2 {
        return Ok(VarValue::Int(1));
    }

    let s = args[0].to_string();
    for a in args[1..].iter() {
        let what = a.to_string();
        if s.find(&what).is_some() {
            return Ok(VarValue::Int(1));
        }
    }
    Ok(VarValue::Int(0))
}

fn replace(args: &[VarValue]) -> FuncResult {
    if args.len() < 2 {
        return Err("requires two arguments".to_string());
    }

    let s = args[0].to_string();
    let what = args[1].to_string();
    let with = if args.len() > 2 {
        args[2].to_string()
    } else {
        String::new()
    };
    Ok(VarValue::Str(s.replace(&what, &with)))
}

fn match_regex(args: &[VarValue]) -> FuncResult {
    if args.len() < 2 {
        return Ok(VarValue::from(1));
    }

    let s = args[0].to_string();
    for a in args[1..].iter() {
        let rx = a.to_string();
        match Regex::new(&rx) {
            Err(e) => return Err(e.to_string()),
            Ok(r) => if r.is_match(&s) {
                return Ok(VarValue::Int(1));
            },
        }
    }
    Ok(VarValue::Int(0))
}

fn pad(args: &[VarValue], loc: Where) -> FuncResult {
    if args.len() < 3 {
        return Err("requires three arguments".to_string());
    }

    let patt = args[1].to_string();
    let patt_width = patt.width() as usize;
    if patt_width == 0 {
        return Err("pad string cannot be empty".to_string());
    }
    let l = args[2].to_int() as usize;
    let s = args[0].to_string();
    let orig_width = s.width() as usize;

    if orig_width + patt_width >= l {
        return Ok(VarValue::from(s));
    }

    let cnt = (l - orig_width) / patt_width;

    let res = match loc {
        Where::All => {
            let right = cnt / 2;
            let left = cnt - right;
            patt.repeat(left) + &s + &patt.repeat(right)
        },
        Where::Left => {
            patt.repeat(cnt) + &s
        },
        Where::Right => {
            s + &patt.repeat(cnt)
        }
    };
    Ok(VarValue::from(res))
}

#[cfg(test)]
mod path_test {
    use super::*;

    #[test]
    fn extract() {
        let v = vec![VarValue::from("c:\\tmp\\file.abc")];
        let r = extract_part(&v, PathPart::Ext);
        assert_eq!(r, Ok(VarValue::from("abc")));
        let r = extract_part(&v, PathPart::Stem);
        assert_eq!(r, Ok(VarValue::from("file")));
        let r = extract_part(&v, PathPart::Name);
        assert_eq!(r, Ok(VarValue::from("file.abc")));
        let r = extract_part(&v, PathPart::Dir);
        assert_eq!(r, Ok(VarValue::from("c:\\tmp")));
    }

    #[test]
    fn change_ext() {
        let v = vec![VarValue::from("c:\\tmp\\file.abc"), VarValue::Str(String::new())];
        let r = replace_ext(&v);
        assert_eq!(r, Ok(VarValue::from("c:\\tmp\\file")));
        let v = vec![VarValue::from("c:\\tmp\\file.abc"), VarValue::from("def")];
        let r = replace_ext(&v);
        assert_eq!(r, Ok(VarValue::from("c:\\tmp\\file.def")));
        let v = vec![VarValue::from("c:\\tmp\\file.abc")];
        let r = replace_ext(&v);
        assert_eq!(r, Ok(VarValue::from("c:\\tmp\\file.abc")));
    }

    #[test]
    fn append_ext() {
        let v = vec![VarValue::from("c:\\tmp\\file.abc"), VarValue::Str(String::new())];
        let r = add_ext(&v);
        assert_eq!(r, Ok(VarValue::from("c:\\tmp\\file.abc")));
        let v = vec![VarValue::from("c:\\tmp\\file.abc"), VarValue::from("def")];
        let r = add_ext(&v);
        assert_eq!(r, Ok(VarValue::from("c:\\tmp\\file.abc.def")));
        let v = vec![VarValue::from("c:\\tmp\\file.abc")];
        let r = add_ext(&v);
        assert_eq!(r, Ok(VarValue::from("c:\\tmp\\file.abc")));
    }

    #[test]
    fn change_name() {
        let v = vec![VarValue::from("c:\\tmp\\file.abc"), VarValue::Str(String::new())];
        let r = replace_name(&v);
        assert_eq!(r, Ok(VarValue::from("c:\\tmp\\")));
        let v = vec![VarValue::from("c:\\tmp\\file.abc"), VarValue::from("some.def")];
        let r = replace_name(&v);
        assert_eq!(r, Ok(VarValue::from("c:\\tmp\\some.def")));
        let v = vec![VarValue::from("c:\\tmp\\file.abc")];
        let r = replace_name(&v);
        assert!(r.is_err());
    }

    #[test]
    fn change_stem() {
        let v = vec![VarValue::from("c:\\tmp\\file.abc"), VarValue::Str(String::new())];
        let r = replace_stem(&v);
        assert!(r.is_err());
        let v = vec![VarValue::from("c:\\tmp\\file.abc"), VarValue::from("some.def")];
        let r = replace_stem(&v);
        assert_eq!(r, Ok(VarValue::from("c:\\tmp\\some.def.abc")));
        let v = vec![VarValue::from("c:\\tmp\\file.abc"), VarValue::from("some")];
        let r = replace_stem(&v);
        assert_eq!(r, Ok(VarValue::from("c:\\tmp\\some.abc")));
        let v = vec![VarValue::from("c:\\tmp\\file.abc")];
        let r = replace_stem(&v);
        assert!(r.is_err());
    }

    #[test]
    fn trims() {
        let v = vec![VarValue::from(" \n abc\t   ")];
        let r = trim_string(&v, Where::All);
        assert_eq!(r, Ok(VarValue::from("abc")));
        let r = trim_string(&v, Where::Left);
        assert_eq!(r, Ok(VarValue::from("abc\t   ")));
        let r = trim_string(&v, Where::Right);
        assert_eq!(r, Ok(VarValue::from(" \n abc")));

        let v = vec![VarValue::from("++abc==="), VarValue::from("+")];
        let r = trim_string(&v, Where::All);
        assert_eq!(r, Ok(VarValue::from("abc===")));
        let v = vec![VarValue::from("++abc==="), VarValue::from("=")];
        let r = trim_string(&v, Where::All);
        assert_eq!(r, Ok(VarValue::from("++abc")));

        let v = vec![VarValue::from("++abc==="), VarValue::from("+")];
        let r = trim_string(&v, Where::Left);
        assert_eq!(r, Ok(VarValue::from("abc===")));
        let r = trim_string(&v, Where::Right);
        assert_eq!(r, Ok(VarValue::from("++abc===")));
    }

    #[test]
    fn end_start() {
        let v = vec![VarValue::from("testabc")];
        let r = starts_with(&v);
        assert_eq!(r, Ok(VarValue::Int(1)));
        let r = ends_with(&v);
        assert_eq!(r, Ok(VarValue::Int(1)));
        let v = vec![VarValue::from("testabc"), VarValue::from("test")];
        let r = starts_with(&v);
        assert_eq!(r, Ok(VarValue::Int(1)));
        let r = ends_with(&v);
        assert_eq!(r, Ok(VarValue::Int(0)));
        let v = vec![VarValue::from("testabc"), VarValue::from("abc")];
        let r = starts_with(&v);
        assert_eq!(r, Ok(VarValue::Int(0)));
        let r = ends_with(&v);
        assert_eq!(r, Ok(VarValue::Int(1)));
        let v = vec![VarValue::from("testabc"), VarValue::from("xxx")];
        let r = starts_with(&v);
        assert_eq!(r, Ok(VarValue::Int(0)));
        let r = ends_with(&v);
        assert_eq!(r, Ok(VarValue::Int(0)));
        let v = vec![VarValue::from("testabc"), VarValue::from("")];
        let r = starts_with(&v);
        assert_eq!(r, Ok(VarValue::Int(1)));
        let r = ends_with(&v);
        assert_eq!(r, Ok(VarValue::Int(1)));
        let v = vec![VarValue::from("testabc"), VarValue::from("test"), VarValue::from("abc")];
        let r = starts_with(&v);
        assert_eq!(r, Ok(VarValue::Int(1)));
        let r = ends_with(&v);
        assert_eq!(r, Ok(VarValue::Int(1)));
    }

    #[test]
    fn up_low() {
        let v = vec![VarValue::from("aBc DeF")];
        let r = change_case(&v, StrCase::Low);
        assert_eq!(r, Ok(VarValue::from("abc def")));
        let r = change_case(&v, StrCase::Up);
        assert_eq!(r, Ok(VarValue::from("ABC DEF")));
    }

    #[test]
    fn contain() {
        let v = vec![VarValue::from("aBc DeF")];
        let r = contains(&v);
        assert_eq!(r, Ok(VarValue::Int(1)));
        let v = vec![VarValue::from("aBc DeF"), VarValue::from("Bc")];
        let r = contains(&v);
        assert_eq!(r, Ok(VarValue::Int(1)));
        let v = vec![VarValue::from("aBc DeF"), VarValue::from("bc")];
        let r = contains(&v);
        assert_eq!(r, Ok(VarValue::Int(0)));
        let v = vec![VarValue::from("aBc DeF"), VarValue::from("bc"), VarValue::from("eF")];
        let r = contains(&v);
        assert_eq!(r, Ok(VarValue::Int(1)));
    }

    #[test]
    fn replaces() {
        let v = vec![VarValue::from("aBc DeF")];
        let r = replace(&v);
        assert!(r.is_err());
        let v = vec![VarValue::from("abc def"), VarValue::from("bc")];
        let r = replace(&v);
        assert_eq!(r, Ok(VarValue::from("a def")));
        let v = vec![VarValue::from("abc def"), VarValue::from("Bc")];
        let r = replace(&v);
        assert_eq!(r, Ok(VarValue::from("abc def")));
        let v = vec![VarValue::from("abc def"), VarValue::from("bc"), VarValue::from("eFG")];
        let r = replace(&v);
        assert_eq!(r, Ok(VarValue::from("aeFG def")));
    }

    #[test]
    fn matches() {
        let v = vec![VarValue::from("aBc DeF")];
        let r = match_regex(&v);
        assert_eq!(r, Ok(VarValue::Int(1)));
        let v = vec![VarValue::from("abc def"), VarValue::from("bc")];
        let r = match_regex(&v);
        assert_eq!(r, Ok(VarValue::from(1)));
        let v = vec![VarValue::from("abc def"), VarValue::from("b.*e")];
        let r = match_regex(&v);
        assert_eq!(r, Ok(VarValue::from(1)));
        let v = vec![VarValue::from("abc def"), VarValue::from("b.*g")];
        let r = match_regex(&v);
        assert_eq!(r, Ok(VarValue::from(0)));
        let v = vec![VarValue::from("abc def"), VarValue::from("b.*g"), VarValue::from("d[mge]+")];
        let r = match_regex(&v);
        assert_eq!(r, Ok(VarValue::from(1)));
    }

    #[test]
    fn pads() {
        let v = vec![VarValue::from("abc")];
        let r = pad(&v, Where::All);
        assert!(r.is_err());
        let v = vec![VarValue::from("abc"), VarValue::from("+=")];
        let r = pad(&v, Where::All);
        assert!(r.is_err());
        let v = vec![VarValue::from("abc"), VarValue::from("")];
        let r = pad(&v, Where::All);
        assert!(r.is_err());

        let v = vec![VarValue::from("abc"), VarValue::from("+="), VarValue::from("aa")];
        let r = pad(&v, Where::All);
        assert_eq!(r, Ok(VarValue::from("abc")));
        let v = vec![VarValue::from("abc"), VarValue::from("+="), VarValue::from(0)];
        let r = pad(&v, Where::All);
        assert_eq!(r, Ok(VarValue::from("abc")));
        let v = vec![VarValue::from("abc"), VarValue::from("+="), VarValue::from(2)];
        let r = pad(&v, Where::All);
        assert_eq!(r, Ok(VarValue::from("abc")));
        let v = vec![VarValue::from("abc"), VarValue::from("+="), VarValue::from(10)];
        let r = pad(&v, Where::All);
        assert_eq!(r, Ok(VarValue::from("+=+=abc+=")));
        let r = pad(&v, Where::Left);
        assert_eq!(r, Ok(VarValue::from("+=+=+=abc")));
        let r = pad(&v, Where::Right);
        assert_eq!(r, Ok(VarValue::from("abc+=+=+=")));

        let v = vec![VarValue::from("abc"), VarValue::from("+="), VarValue::from(11)];
        let r = pad(&v, Where::All);
        assert_eq!(r, Ok(VarValue::from("+=+=abc+=+=")));
    }
}
