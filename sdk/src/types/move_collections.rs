use {
    crate::sui,
    serde::{Deserialize, Serialize},
    std::marker::PhantomData,
};

/// Move `sui::vec_set::VecSet<T>`.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct MoveVecSet<T> {
    pub contents: Vec<T>,
}

/// Move `sui::table::Table<K, V>`.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct MoveTable<K, V> {
    pub id: sui::types::Address,
    pub size: u64,
    #[serde(skip)]
    _marker: PhantomData<(K, V)>,
}

impl<K, V> MoveTable<K, V> {
    pub fn new(id: sui::types::Address, size: u64) -> Self {
        Self {
            id,
            size,
            _marker: PhantomData,
        }
    }

    pub fn size(&self) -> usize {
        usize::try_from(self.size).unwrap_or(usize::MAX)
    }
}
