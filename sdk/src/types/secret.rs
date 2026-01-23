use zeroize::Zeroize;

/// A redacted, zeroized wrapper for secret values.
///
/// This intentionally does not implement `Deref` to reduce accidental leakage
/// via APIs that might log or format values.
pub struct Secret<T: Zeroize>(zeroize::Zeroizing<T>);

impl<T: Zeroize> Secret<T> {
    pub fn new(value: T) -> Self {
        Self(zeroize::Zeroizing::new(value))
    }

    pub fn expose(&self) -> &T {
        &self.0
    }

    pub fn expose_mut(&mut self) -> &mut T {
        &mut self.0
    }

    /// Move out the wrapped value while preserving zeroization-on-drop.
    pub fn into_zeroizing(self) -> zeroize::Zeroizing<T> {
        self.0
    }
}

impl<T: Zeroize> From<T> for Secret<T> {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

impl<T: Zeroize + Clone> Clone for Secret<T> {
    fn clone(&self) -> Self {
        Self::new(self.expose().clone())
    }
}

impl<T: Zeroize + PartialEq> PartialEq for Secret<T> {
    fn eq(&self, other: &Self) -> bool {
        self.expose() == other.expose()
    }
}

impl<T: Zeroize + Eq> Eq for Secret<T> {}

impl<T: Zeroize> std::fmt::Debug for Secret<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Secret([Redacted: *****])")
    }
}

impl<T: Zeroize> std::fmt::Display for Secret<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[Redacted: *****]")
    }
}
