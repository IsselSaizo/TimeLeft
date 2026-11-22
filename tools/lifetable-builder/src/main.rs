//! Offline tool: UN WPP 2024 complete life tables (CSV.gz) -> `data/lifetable.bin`.
//!
//! ```text
//! cargo run -p lifetable-builder --release -- [--cache DIR] [--out FILE] [--reference FILE]
//! ```
//! Missing input files are downloaded with `curl` into the cache directory (~400 MB).
//! The CSVs are streamed, only the anchor years are kept.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use flate2::read::MultiGzDecoder;
use timeleft_core::lifetable::{self, LifeTables, Location, N_AGE, N_SEX, RawLocation};

const BASE_URL: &str =
    "https://population.un.org/wpp/assets/Excel%20Files/1_Indicator%20(Standard)/CSV_FILES/";
const FILES: [(usize, &str); 2] = [
    (
        0,
        "WPP2024_Life_Table_Complete_Medium_Male_2024-2100.csv.gz",
    ),
    (
        1,
        "WPP2024_Life_Table_Complete_Medium_Female_2024-2100.csv.gz",
    ),
];
const ANCHORS: [u16; 3] = [2024, 2050, 2100];
const SOURCE: &str = "United Nations, DESA, Population Division (2024). World Population Prospects 2024, \
Complete life tables, Medium variant. CC BY 3.0 IGO.";
const WORLD_LOC_ID: u16 = 900;
/// Countries in the reference file used by the core tests.
const REFERENCE_ISO3: [&str; 10] = [
    "WLD", "RUS", "TUR", "USA", "JPN", "DEU", "GBR", "IND", "NGA", "BRA",
];
const REFERENCE_AGES: [usize; 7] = [0, 20, 40, 60, 80, 90, 100];

struct Acc {
    location: Location,
    mx: Vec<f64>,
    filled: Vec<bool>,
    /// ex in the base year, `[sex][age]`.
    ex_base: Vec<f64>,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut cache = root.join("tools/lifetable-builder/cache");
    let mut out = root.join("data/lifetable.bin");
    let mut reference = root.join("testdata/wpp_reference.json");
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let mut val = || {
            args.next()
                .map(PathBuf::from)
                .ok_or(format!("{a} needs a value"))
        };
        match a.as_str() {
            "--cache" => cache = val()?,
            "--out" => out = val()?,
            "--reference" => reference = val()?,
            "-h" | "--help" => {
                println!("lifetable-builder [--cache DIR] [--out FILE] [--reference FILE]");
                return Ok(());
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    std::fs::create_dir_all(&cache).map_err(|e| e.to_string())?;

    let mut locs: BTreeMap<u16, Acc> = BTreeMap::new();
    for (sex, name) in FILES {
        let path = cache.join(name);
        if !path.exists() {
            download(name, &path)?;
        }
        eprintln!("reading {name} ...");
        read_file(&path, sex, &mut locs)?;
    }

    let per_loc = N_SEX * ANCHORS.len() * N_AGE;
    let reference_ex: BTreeMap<String, Vec<f64>> = locs
        .values()
        .filter(|a| REFERENCE_ISO3.contains(&a.location.iso3.as_str()))
        .map(|a| (a.location.iso3.clone(), a.ex_base.clone()))
        .collect();
    if locs.len() < 200 {
        return Err(format!(
            "only {} locations parsed, input looks wrong",
            locs.len()
        ));
    }
    let mut raw: Vec<RawLocation> = Vec::new();
    for acc in locs.into_values() {
        let missing = acc.filled.iter().filter(|f| !**f).count();
        if missing > 0 {
            eprintln!(
                "skip {} ({}): {missing}/{per_loc} values missing",
                acc.location.iso3, acc.location.name
            );
            continue;
        }
        if let Some(bad) = acc.mx.iter().find(|v| !(**v > 0.0 && v.is_finite())) {
            eprintln!("skip {}: invalid mx {bad}", acc.location.iso3);
            continue;
        }
        raw.push(RawLocation {
            location: acc.location,
            mx: acc.mx,
        });
    }
    raw.sort_by(|a, b| a.location.iso3.cmp(&b.location.iso3));
    if raw
        .windows(2)
        .any(|w| w[0].location.iso3 == w[1].location.iso3)
    {
        return Err("duplicate ISO3 codes".into());
    }

    let bin = lifetable::encode(&ANCHORS, &raw, SOURCE);
    LifeTables::decode(&bin).map_err(|e| format!("self-check failed: {e}"))?;
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(&out, &bin).map_err(|e| e.to_string())?;
    eprintln!(
        "wrote {} ({} locations, {} bytes)",
        out.display(),
        raw.len(),
        bin.len()
    );
    write_reference(&reference, &reference_ex)
}

fn download(name: &str, dest: &Path) -> Result<(), String> {
    eprintln!("downloading {name} ...");
    let part = dest.with_extension("part");
    let status = Command::new("curl")
        .args(["-fL", "--retry", "5", "-C", "-", "-o"])
        .arg(&part)
        .arg(format!("{BASE_URL}{name}"))
        .status()
        .map_err(|e| format!("cannot run curl: {e}"))?;
    if !status.success() {
        return Err(format!("curl failed for {name}: {status}"));
    }
    std::fs::rename(&part, dest).map_err(|e| e.to_string())
}

struct Columns {
    loc_id: usize,
    iso3: usize,
    loc_type: usize,
    location: usize,
    time: usize,
    age_start: usize,
    mx: usize,
    ex: usize,
}

type CsvReader = csv::Reader<MultiGzDecoder<BufReader<File>>>;

fn open(path: &Path) -> Result<(CsvReader, Columns), String> {
    let f = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut rdr = csv::ReaderBuilder::new()
        .from_reader(MultiGzDecoder::new(BufReader::with_capacity(1 << 20, f)));
    let headers = rdr.headers().map_err(|e| e.to_string())?.clone();
    let col = |n: &str| {
        headers
            .iter()
            .position(|h| h.trim_start_matches('\u{feff}') == n)
            .ok_or(format!("column {n} not found"))
    };
    let cols = Columns {
        loc_id: col("LocID")?,
        iso3: col("ISO3_code")?,
        loc_type: col("LocTypeID")?,
        location: col("Location")?,
        time: col("Time")?,
        age_start: col("AgeGrpStart")?,
        mx: col("mx")?,
        ex: col("ex")?,
    };
    Ok((rdr, cols))
}

fn read_file(path: &Path, sex: usize, locs: &mut BTreeMap<u16, Acc>) -> Result<(), String> {
    let (mut rdr, c) = open(path)?;
    let n_anchor = ANCHORS.len();
    let mut rec = csv::StringRecord::new();
    let mut rows = 0u64;
    while rdr.read_record(&mut rec).map_err(|e| e.to_string())? {
        rows += 1;
        let Ok(year) = rec[c.time].parse::<u16>() else {
            continue;
        };
        let Some(anchor) = ANCHORS.iter().position(|&a| a == year) else {
            continue;
        };
        let loc_id: u32 = rec[c.loc_id]
            .parse()
            .map_err(|_| format!("bad LocID at row {rows}"))?;
        let is_country = &rec[c.loc_type] == "4" && rec[c.iso3].len() == 3;
        if !is_country && loc_id != WORLD_LOC_ID as u32 {
            continue;
        }
        let loc_id = u16::try_from(loc_id).map_err(|_| format!("LocID {loc_id} too large"))?;
        let age: usize = rec[c.age_start]
            .parse()
            .map_err(|_| format!("bad age at row {rows}"))?;
        if age >= N_AGE {
            continue;
        }
        let mx: f64 = rec[c.mx]
            .parse()
            .map_err(|_| format!("bad mx at row {rows}"))?;
        let ex: f64 = rec[c.ex].parse().unwrap_or(f64::NAN);
        let acc = locs.entry(loc_id).or_insert_with(|| Acc {
            location: Location {
                iso3: if loc_id == WORLD_LOC_ID {
                    "WLD".into()
                } else {
                    rec[c.iso3].to_string()
                },
                loc_id,
                name: rec[c.location].to_string(),
            },
            mx: vec![0.0; N_SEX * n_anchor * N_AGE],
            filled: vec![false; N_SEX * n_anchor * N_AGE],
            ex_base: vec![f64::NAN; N_SEX * N_AGE],
        });
        let i = (sex * n_anchor + anchor) * N_AGE + age;
        acc.mx[i] = mx;
        acc.filled[i] = true;
        if anchor == 0 {
            acc.ex_base[sex * N_AGE + age] = ex;
        }
    }
    eprintln!("  {rows} rows");
    Ok(())
}

fn write_reference(path: &Path, ex: &BTreeMap<String, Vec<f64>>) -> Result<(), String> {
    if let Some(missing) = REFERENCE_ISO3.iter().find(|i| !ex.contains_key(**i)) {
        return Err(format!("reference country {missing} not found"));
    }
    let mut countries = serde_json::Map::new();
    for (iso, by_sex) in ex {
        let mut sexes = serde_json::Map::new();
        for (sex, label) in [(0, "male"), (1, "female")] {
            let v = &by_sex[sex * N_AGE..(sex + 1) * N_AGE];
            if v.iter().any(|x| x.is_nan()) {
                return Err(format!("reference {iso}/{label}: missing ex"));
            }
            let ages: serde_json::Map<_, _> = REFERENCE_AGES
                .iter()
                .map(|&a| (a.to_string(), serde_json::json!(v[a])))
                .collect();
            sexes.insert(label.into(), ages.into());
        }
        countries.insert(iso.clone(), sexes.into());
    }
    let doc = serde_json::json!({
        "description": "Period life expectancy e(x) from UN WPP 2024 (Medium), used to validate the core model",
        "year": ANCHORS[0],
        "source": SOURCE,
        "ex": countries,
    });
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())? + "\n";
    std::fs::write(path, text).map_err(|e| e.to_string())?;
    eprintln!("wrote {}", path.display());
    Ok(())
}
