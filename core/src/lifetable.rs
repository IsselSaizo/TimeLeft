//! Compact life-table storage (`data/lifetable.bin`).
//!
//! The file holds single-age death rates `mx` (ages 0..=100, where 100 is the open
//! interval 100+) for every location, both sexes and a few anchor years of the UN WPP
//! Medium projection (2024, 2050, 2100). Values between anchors are interpolated
//! log-linearly, which is how the cohort mortality improvement is modelled.
//!
//! Binary layout (all integers little-endian):
//! ```text
//! magic        b"TLLT"
//! version      u16 (= 1)
//! n_loc        u16
//! n_sex        u8  (= 2: 0 male, 1 female)
//! n_age        u8  (= 101)
//! n_anchor     u8
//! reserved     u8
//! anchors      u16 * n_anchor           calendar years
//! ln_min       f32                       quantisation range of ln(mx)
//! ln_max       f32
//! source_len   u16, source utf-8
//! locations    n_loc * { iso3 [u8;3], loc_id u16, name_len u8, name utf-8 }
//! data         n_loc * n_sex * n_anchor * n_age * u16
//! ```

use crate::error::Error;

pub const MAGIC: &[u8; 4] = b"TLLT";
pub const FORMAT_VERSION: u16 = 1;
pub const N_SEX: usize = 2;
pub const N_AGE: usize = 101;
/// Oldest tabulated age (open interval `100+`).
pub const MAX_TABLE_AGE: usize = N_AGE - 1;

const LN_MIN: f32 = -13.815_511; // ln(1e-6)
const LN_MAX: f32 = 1.386_294_4; // ln(4)

/// Biological sex used to select the life table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Sex {
    Male,
    Female,
    /// Average of the male and female tables.
    Unspecified,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Location {
    pub iso3: String,
    pub loc_id: u16,
    pub name: String,
}

/// Uncompressed input for [`encode`]: `mx[sex][anchor][age]` flattened.
#[derive(Debug, Clone)]
pub struct RawLocation {
    pub location: Location,
    /// Length `N_SEX * anchors.len() * N_AGE`.
    pub mx: Vec<f64>,
}

#[derive(Debug, Clone)]
pub struct LifeTables {
    anchors: Vec<u16>,
    source: String,
    locations: Vec<Location>,
    /// ln(mx), `[loc][sex][anchor][age]`.
    ln_mx: Vec<f32>,
}

fn quantise(mx: f64) -> u16 {
    let ln = (mx.max(1e-12)).ln() as f32;
    let x = ((ln - LN_MIN) / (LN_MAX - LN_MIN)).clamp(0.0, 1.0);
    (x * u16::MAX as f32).round() as u16
}

fn dequantise(q: u16) -> f32 {
    LN_MIN + (q as f32 / u16::MAX as f32) * (LN_MAX - LN_MIN)
}

/// Serialise life tables into the `lifetable.bin` format.
pub fn encode(anchors: &[u16], locations: &[RawLocation], source: &str) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    out.extend_from_slice(&(locations.len() as u16).to_le_bytes());
    out.push(N_SEX as u8);
    out.push(N_AGE as u8);
    out.push(anchors.len() as u8);
    out.push(0);
    for a in anchors {
        out.extend_from_slice(&a.to_le_bytes());
    }
    out.extend_from_slice(&LN_MIN.to_le_bytes());
    out.extend_from_slice(&LN_MAX.to_le_bytes());
    out.extend_from_slice(&(source.len() as u16).to_le_bytes());
    out.extend_from_slice(source.as_bytes());
    for l in locations {
        let iso = l.location.iso3.as_bytes();
        assert_eq!(iso.len(), 3, "iso3 must be 3 ASCII letters");
        out.extend_from_slice(iso);
        out.extend_from_slice(&l.location.loc_id.to_le_bytes());
        let name = l.location.name.as_bytes();
        let name = &name[..name.len().min(255)];
        out.push(name.len() as u8);
        out.extend_from_slice(name);
    }
    let per_loc = N_SEX * anchors.len() * N_AGE;
    for l in locations {
        assert_eq!(l.mx.len(), per_loc, "mx length for {}", l.location.iso3);
        for &v in &l.mx {
            out.extend_from_slice(&quantise(v).to_le_bytes());
        }
    }
    out
}

struct Reader<'a> {
    b: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], Error> {
        let end = self.pos.checked_add(n).filter(|&e| e <= self.b.len());
        let end = end.ok_or_else(|| Error::Data("lifetable.bin truncated".into()))?;
        let s = &self.b[self.pos..end];
        self.pos = end;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8, Error> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, Error> {
        let s = self.take(2)?;
        Ok(u16::from_le_bytes([s[0], s[1]]))
    }
    fn f32(&mut self) -> Result<f32, Error> {
        let s = self.take(4)?;
        Ok(f32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }
    fn str(&mut self, n: usize) -> Result<String, Error> {
        String::from_utf8(self.take(n)?.to_vec()).map_err(|_| Error::Data("invalid utf-8".into()))
    }
}

impl LifeTables {
    /// Decode a `lifetable.bin` image.
    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        let mut r = Reader { b: bytes, pos: 0 };
        if r.take(4)? != MAGIC {
            return Err(Error::Data("not a lifetable.bin file".into()));
        }
        let version = r.u16()?;
        if version != FORMAT_VERSION {
            return Err(Error::Data(format!(
                "unsupported lifetable.bin version {version}"
            )));
        }
        let n_loc = r.u16()? as usize;
        let (n_sex, n_age, n_anchor) = (r.u8()? as usize, r.u8()? as usize, r.u8()? as usize);
        r.u8()?;
        if n_sex != N_SEX || n_age != N_AGE || n_anchor == 0 {
            return Err(Error::Data("unexpected lifetable dimensions".into()));
        }
        let anchors = (0..n_anchor)
            .map(|_| r.u16())
            .collect::<Result<Vec<_>, _>>()?;
        if anchors.windows(2).any(|w| w[0] >= w[1]) {
            return Err(Error::Data("anchor years must increase".into()));
        }
        let (ln_min, ln_max) = (r.f32()?, r.f32()?);
        if ln_min != LN_MIN || ln_max != LN_MAX {
            return Err(Error::Data("unexpected quantisation range".into()));
        }
        let source_len = r.u16()? as usize;
        let source = r.str(source_len)?;
        let mut locations = Vec::with_capacity(n_loc);
        for _ in 0..n_loc {
            let iso3 = r.str(3)?;
            let loc_id = r.u16()?;
            let name_len = r.u8()? as usize;
            let name = r.str(name_len)?;
            locations.push(Location { iso3, loc_id, name });
        }
        let total = n_loc * N_SEX * n_anchor * N_AGE;
        let raw = r.take(total * 2)?;
        let ln_mx = raw
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| dequantise(u16::from_le_bytes(*c)))
            .collect();
        if r.pos != bytes.len() {
            return Err(Error::Data("trailing bytes in lifetable.bin".into()));
        }
        Ok(Self {
            anchors,
            source,
            locations,
            ln_mx,
        })
    }

    /// Tables bundled into the library at compile time.
    #[cfg(feature = "embedded-data")]
    pub fn embedded() -> &'static LifeTables {
        use std::sync::OnceLock;
        static BYTES: &[u8] = include_bytes!("../../data/lifetable.bin");
        static TABLES: OnceLock<LifeTables> = OnceLock::new();
        TABLES.get_or_init(|| LifeTables::decode(BYTES).expect("embedded lifetable.bin is valid"))
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn anchors(&self) -> &[u16] {
        &self.anchors
    }

    /// First calendar year covered by the tables.
    pub fn base_year(&self) -> u16 {
        self.anchors[0]
    }

    pub fn locations(&self) -> &[Location] {
        &self.locations
    }

    /// Index of a location by ISO 3166-1 alpha-3 code (case-insensitive).
    pub fn find(&self, iso3: &str) -> Option<usize> {
        self.locations
            .iter()
            .position(|l| l.iso3.eq_ignore_ascii_case(iso3))
    }

    fn ln_at(&self, loc: usize, sex: usize, anchor: usize, age: usize) -> f64 {
        let n_anchor = self.anchors.len();
        self.ln_mx[((loc * N_SEX + sex) * n_anchor + anchor) * N_AGE + age] as f64
    }

    fn ln_mx_sex(&self, loc: usize, sex: usize, age: usize, year: f64) -> f64 {
        let a = &self.anchors;
        let last = a.len() - 1;
        if year <= a[0] as f64 {
            return self.ln_at(loc, sex, 0, age);
        }
        if year >= a[last] as f64 {
            return self.ln_at(loc, sex, last, age);
        }
        let i = a
            .windows(2)
            .position(|w| year < w[1] as f64)
            .unwrap_or(last - 1);
        let (y0, y1) = (a[i] as f64, a[i + 1] as f64);
        let t = (year - y0) / (y1 - y0);
        let (l0, l1) = (
            self.ln_at(loc, sex, i, age),
            self.ln_at(loc, sex, i + 1, age),
        );
        l0 + (l1 - l0) * t
    }

    /// Central death rate at single age `age` (0..=100; 100 means 100+) in calendar `year`.
    /// Years before the first anchor use the first anchor, after the last — the last one.
    pub fn mx(&self, loc: usize, sex: Sex, age: usize, year: f64) -> f64 {
        let age = age.min(MAX_TABLE_AGE);
        match sex {
            Sex::Male => self.ln_mx_sex(loc, 0, age, year).exp(),
            Sex::Female => self.ln_mx_sex(loc, 1, age, year).exp(),
            Sex::Unspecified => {
                0.5 * (self.ln_mx_sex(loc, 0, age, year).exp()
                    + self.ln_mx_sex(loc, 1, age, year).exp())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic() -> LifeTables {
        let anchors = [2024u16, 2050, 2100];
        let mut mx = Vec::new();
        for sex in 0..N_SEX {
            for (ai, _) in anchors.iter().enumerate() {
                for age in 0..N_AGE {
                    let base = 1e-4 * (0.09 * age as f64).exp() * if sex == 0 { 1.3 } else { 1.0 };
                    mx.push(base * 0.8f64.powi(ai as i32));
                }
            }
        }
        let loc = RawLocation {
            location: Location {
                iso3: "TST".into(),
                loc_id: 1,
                name: "Testland".into(),
            },
            mx,
        };
        LifeTables::decode(&encode(&anchors, &[loc], "synthetic")).unwrap()
    }

    #[test]
    fn roundtrip_precision() {
        let t = synthetic();
        assert_eq!(t.find("tst"), Some(0));
        let v = t.mx(0, Sex::Female, 40, 2024.0);
        let expect = 1e-4 * (0.09f64 * 40.0).exp();
        assert!((v / expect - 1.0).abs() < 5e-4, "{v} vs {expect}");
    }

    #[test]
    fn log_linear_interpolation() {
        let t = synthetic();
        let a = t.mx(0, Sex::Male, 60, 2024.0);
        let b = t.mx(0, Sex::Male, 60, 2050.0);
        let mid = t.mx(0, Sex::Male, 60, 2037.0);
        assert!((mid / (a * b).sqrt() - 1.0).abs() < 1e-3);
        assert_eq!(t.mx(0, Sex::Male, 60, 1990.0), a);
        assert_eq!(
            t.mx(0, Sex::Male, 60, 2200.0),
            t.mx(0, Sex::Male, 60, 2100.0)
        );
    }

    #[test]
    fn rejects_garbage() {
        assert!(LifeTables::decode(b"nope").is_err());
        let t = encode(&[2024], &[], "x");
        assert!(LifeTables::decode(&t[..t.len() - 1]).is_err());
    }
}
