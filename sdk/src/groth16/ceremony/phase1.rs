use {
    crate::groth16::{curve::PairingEngine, error::SetupError},
    ark_serialize::{CanonicalDeserialize, Compress, Validate},
    std::{
        fs::File,
        io::{BufReader, Read},
        path::Path,
    },
};

/// Phase-1 universal SRS: [tau^i] in G1 and G2.
/// We require at least `n_g1` and `n_g2` powers respectively.
pub struct Phase1<E: PairingEngine> {
    pub g1_powers: Vec<E::G1Affine>,
    pub g2_powers: Vec<E::G2Affine>,
}

impl<E: PairingEngine> Phase1<E> {
    pub fn ensure_min_powers(&self, need_g1: usize, need_g2: usize) -> Result<(), SetupError> {
        if self.g1_powers.len() < need_g1 {
            return Err(SetupError::Format("insufficient G1 powers"));
        }
        if self.g2_powers.len() < need_g2 {
            return Err(SetupError::Format("insufficient G2 powers"));
        }
        Ok(())
    }
}

/// Abstract source of Phase-1 data
pub trait Phase1Source<E: PairingEngine> {
    fn load(&self) -> Result<Phase1<E>, SetupError>;
}

/// Zcash-hosted phase1radix2mX helper.
pub struct ZcashRadix2m<'a> {
    pub path: &'a Path,       // local file
    pub expect_min_g1: usize, // sanity bounds
    pub expect_min_g2: usize,
}

impl<E: PairingEngine> Phase1Source<E> for ZcashRadix2m<'_> {
    fn load(&self) -> Result<Phase1<E>, SetupError> {
        let mut f = BufReader::new(File::open(self.path)?);

        let mut buf = Vec::new();
        f.read_to_end(&mut buf)?;
        let rdr = &buf[..];

        // Try to parse (best-effort). If it fails, return a format error with guidance.
        let try_parse = || -> Option<Phase1<E>> {
            let mut off = 0usize;

            // naive helper to read a u32 LE
            let read_u32 = |b: &[u8], off: &mut usize| -> Option<u32> {
                if *off + 4 > b.len() {
                    return None;
                }
                let v = u32::from_le_bytes(b[*off..*off + 4].try_into().ok()?);
                *off += 4;
                Some(v)
            };

            let g1_len = read_u32(rdr, &mut off)? as usize;
            let mut g1 = Vec::with_capacity(g1_len);
            for _ in 0..g1_len {
                let mut cursor = std::io::Cursor::new(&rdr[off..]);
                let pt = <E::G1Affine as CanonicalDeserialize>::deserialize_with_mode(
                    &mut cursor,
                    Compress::Yes,
                    Validate::Yes,
                )
                .ok()?;
                off += cursor.position() as usize;
                g1.push(pt);
            }

            let g2_len = read_u32(rdr, &mut off)? as usize;
            let mut g2 = Vec::with_capacity(g2_len);
            for _ in 0..g2_len {
                let mut cursor = std::io::Cursor::new(&rdr[off..]);
                let pt = <E::G2Affine as CanonicalDeserialize>::deserialize_with_mode(
                    &mut cursor,
                    Compress::Yes,
                    Validate::Yes,
                )
                .ok()?;
                off += cursor.position() as usize;
                g2.push(pt);
            }
            Some(Phase1 {
                g1_powers: g1,
                g2_powers: g2,
            })
        };

        if let Some(p1) = try_parse() {
            p1.ensure_min_powers(self.expect_min_g1, self.expect_min_g2)?;
            return Ok(p1);
        }

        Err(SetupError::Format(
            "Unsupported phase1radix2mX binary layout. Plug your parser here (e.g., bellman).",
        ))
    }
}
