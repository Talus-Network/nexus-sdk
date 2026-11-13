use {crate::move_bindings::interface::verifier::RegisteredKeyAuxiliary, anyhow::bail};

impl RegisteredKeyAuxiliary {
    /// Validates the fixed lengths of the commitments and signatures.
    pub fn validate(&self) -> anyhow::Result<()> {
        for (field, bytes, expected) in [
            ("input hash", self.input_hash.as_slice(), 32),
            ("nonce", self.nonce.as_slice(), 32),
            ("leader signature", self.leader_signature.as_slice(), 64),
            ("tool signature", self.tool_signature.as_slice(), 64),
        ] {
            if bytes.len() != expected {
                bail!("RegisteredKey {field} must be {expected} bytes");
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registered_key_auxiliary_requires_exact_field_lengths() {
        let valid = RegisteredKeyAuxiliary::new(vec![1; 32], vec![2; 32], vec![3; 64], vec![4; 64]);
        assert!(valid.validate().is_ok());

        for lengths in [
            [31, 32, 64, 64],
            [33, 32, 64, 64],
            [32, 31, 64, 64],
            [32, 33, 64, 64],
            [32, 32, 63, 64],
            [32, 32, 65, 64],
            [32, 32, 64, 63],
            [32, 32, 64, 65],
        ] {
            let invalid = RegisteredKeyAuxiliary::new(
                vec![1; lengths[0]],
                vec![2; lengths[1]],
                vec![3; lengths[2]],
                vec![4; lengths[3]],
            );
            assert!(invalid.validate().is_err(), "accepted lengths {lengths:?}");
        }
    }
}
