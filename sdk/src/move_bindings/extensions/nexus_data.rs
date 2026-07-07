//! Constructors and accessors for generated Nexus data values.
//!
//! [`crate::move_bindings::primitives::data::NexusData`] is persisted as raw storage tag bytes
//! plus one or many payload byte arrays. These helpers name the tag values used by Nexus and make
//! common inline and Walrus layouts explicit without introducing another data container.

const NEXUS_DATA_INLINE_STORAGE_TAG: &[u8] = b"inline";
const NEXUS_DATA_WALRUS_STORAGE_TAG: &[u8] = b"walrus";

impl crate::move_bindings::primitives::data::NexusData {
    pub fn inline_one(data: impl Into<Vec<u8>>) -> Self {
        Self::from_parts(NEXUS_DATA_INLINE_STORAGE_TAG, data.into(), Vec::new())
    }

    pub fn inline_many<I, B>(many: I) -> Self
    where
        I: IntoIterator<Item = B>,
        B: Into<Vec<u8>>,
    {
        Self::from_parts(
            NEXUS_DATA_INLINE_STORAGE_TAG,
            Vec::new(),
            many.into_iter().map(Into::into).collect(),
        )
    }

    pub fn walrus_one(data: impl Into<Vec<u8>>) -> Self {
        Self::from_parts(NEXUS_DATA_WALRUS_STORAGE_TAG, data.into(), Vec::new())
    }

    pub fn walrus_many<I, B>(many: I) -> Self
    where
        I: IntoIterator<Item = B>,
        B: Into<Vec<u8>>,
    {
        Self::from_parts(
            NEXUS_DATA_WALRUS_STORAGE_TAG,
            Vec::new(),
            many.into_iter().map(Into::into).collect(),
        )
    }

    pub fn inline_one_bytes(&self) -> Option<&[u8]> {
        (self.storage.as_slice() == NEXUS_DATA_INLINE_STORAGE_TAG && self.many.is_empty())
            .then_some(self.one.as_slice())
    }

    pub fn storage_tag(&self) -> &[u8] {
        self.storage.as_slice()
    }

    pub fn is_inline(&self) -> bool {
        self.storage.as_slice() == NEXUS_DATA_INLINE_STORAGE_TAG
    }

    pub fn is_walrus(&self) -> bool {
        self.storage.as_slice() == NEXUS_DATA_WALRUS_STORAGE_TAG
    }

    fn from_parts(storage: &[u8], one: Vec<u8>, many: Vec<Vec<u8>>) -> Self {
        Self {
            storage: storage.to_vec(),
            one,
            many,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::move_bindings::primitives::data::NexusData;

    #[test]
    fn raw_byte_constructors_match_move_shape() {
        let inline = NexusData::inline_one(b"failure".to_vec());
        assert!(inline.is_inline());
        assert_eq!(inline.inline_one_bytes(), Some(b"failure".as_slice()));
        assert_eq!(inline.one, b"failure".to_vec());
        assert!(inline.many.is_empty());

        let many = NexusData::inline_many([b"left".to_vec(), b"right".to_vec()]);
        assert!(many.is_inline());
        assert!(many.one.is_empty());
        assert_eq!(many.many, vec![b"left".to_vec(), b"right".to_vec()]);

        let walrus = NexusData::walrus_one(b"blob-id".to_vec());
        assert!(walrus.is_walrus());
        assert_eq!(walrus.one, b"blob-id".to_vec());
        assert!(walrus.many.is_empty());
    }
}
