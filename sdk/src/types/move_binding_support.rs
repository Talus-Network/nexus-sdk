//! SDK helper for using movebinding generated types.
//! Need to be fixed in movebinding directly and then removed from here.

use {
    crate::sui,
    serde::{
        de::{DeserializeOwned, Error as _},
        ser::SerializeSeq,
        Deserialize,
        Serialize,
    },
    std::{borrow::Cow, fmt, marker::PhantomData},
    sui_move::{HasCopy, HasDrop, HasStore, MoveStruct, MoveType},
};

/// Ubiquitously used Move type-name wrapper.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct TypeName {
    pub name: String,
}

impl TypeName {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
        }
    }

    fn normalize(name: &str) -> Cow<'_, str> {
        let trimmed = name.trim_start_matches("0x");
        if trimmed.len() == name.len() {
            Cow::Borrowed(name)
        } else {
            Cow::Owned(trimmed.to_string())
        }
    }

    /// Returns true when two fully-qualified Move type names represent the same symbol.
    pub fn matches_qualified_name(&self, expected: &str) -> bool {
        Self::normalize(&self.name).eq_ignore_ascii_case(&Self::normalize(expected))
    }
}

impl From<&str> for TypeName {
    fn from(name: &str) -> Self {
        TypeName::new(name)
    }
}

impl From<String> for TypeName {
    fn from(name: String) -> Self {
        TypeName { name }
    }
}

impl fmt::Display for TypeName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

impl<'de> Deserialize<'de> for TypeName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        if !deserializer.is_human_readable() {
            #[derive(Deserialize)]
            struct RawTypeName {
                name: String,
            }

            return RawTypeName::deserialize(deserializer)
                .map(|value| TypeName { name: value.name });
        }

        let value = serde_json::Value::deserialize(deserializer)?;
        super::move_json::parse_type_name_value(&value)
            .map_err(D::Error::custom)?
            .ok_or_else(|| D::Error::custom("invalid TypeName value"))
    }
}

pub type Bag = crate::types::sui_framework::bag::Bag;
pub type ObjectBag = crate::types::sui_framework::object_bag::ObjectBag;
pub type PriorityQueue<T> = crate::types::sui_framework::priority_queue::PriorityQueue<T>;
pub type MoveTable<K, V> = crate::types::sui_framework::table::Table<K, V>;
pub type MoveVecSet<T> = crate::types::sui_framework::vec_set::VecSet<T>;
pub type ID = crate::types::sui_framework::object::ID;
pub type UID = crate::types::sui_framework::object::UID;

pub fn sui_address_to_id(bytes: sui::types::Address) -> crate::types::sui_framework::object::ID {
    crate::types::sui_framework::object::ID { bytes }
}

pub fn sui_address_to_uid(bytes: sui::types::Address) -> crate::types::sui_framework::object::UID {
    crate::types::sui_framework::object::UID {
        id: sui_address_to_id(bytes),
    }
}

/// Serde bridge that lets generated Sui object IDs keep their Move layout
/// while accepting Sui JSON address strings.
#[derive(Clone, Debug, Serialize)]
pub struct ObjectIdSerde {
    pub bytes: sui::types::Address,
}

impl<'de> Deserialize<'de> for ObjectIdSerde {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct IdRepr {
            bytes: sui::types::Address,
        }

        if !deserializer.is_human_readable() {
            return IdRepr::deserialize(deserializer).map(|value| Self { bytes: value.bytes });
        }

        let value = serde_json::Value::deserialize(deserializer)?;
        if let Some(bytes) =
            super::move_json::parse_address_value(&value).map_err(serde::de::Error::custom)?
        {
            return Ok(Self { bytes });
        }

        serde_json::from_value::<IdRepr>(value)
            .map(|value| Self { bytes: value.bytes })
            .map_err(serde::de::Error::custom)
    }
}

impl From<ObjectIdSerde> for crate::types::sui_framework::object::ID {
    fn from(value: ObjectIdSerde) -> Self {
        sui_address_to_id(value.bytes)
    }
}

impl From<ObjectIdSerde> for crate::types::sui_framework::object::UID {
    fn from(value: ObjectIdSerde) -> Self {
        Self {
            id: sui_address_to_id(value.bytes),
        }
    }
}

impl From<crate::types::sui_framework::object::ID> for ObjectIdSerde {
    fn from(value: crate::types::sui_framework::object::ID) -> Self {
        Self { bytes: value.bytes }
    }
}

impl From<crate::types::sui_framework::object::UID> for ObjectIdSerde {
    fn from(value: crate::types::sui_framework::object::UID) -> Self {
        Self {
            bytes: value.id.bytes,
        }
    }
}

/// Serde bridge that lets generated interface versions keep their Move layout
/// while accepting the scalar JSON form used by TAP APIs and tests.
#[derive(Clone, Debug)]
pub struct InterfaceVersionSerde {
    pub inner: u64,
}

impl<'de> Deserialize<'de> for InterfaceVersionSerde {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        crate::types::serde_parsers::deserialize_tap_u64_value(deserializer)
            .map(|inner| Self { inner })
    }
}

impl Serialize for InterfaceVersionSerde {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_u64(self.inner)
    }
}

impl From<InterfaceVersionSerde> for crate::types::interface::version::InterfaceVersion {
    fn from(value: InterfaceVersionSerde) -> Self {
        Self { inner: value.inner }
    }
}

impl From<crate::types::interface::version::InterfaceVersion> for InterfaceVersionSerde {
    fn from(value: crate::types::interface::version::InterfaceVersion) -> Self {
        Self { inner: value.inner }
    }
}

impl Clone for crate::types::sui_framework::object::UID {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
        }
    }
}

impl<T0, T1> Clone for crate::types::sui_framework::vec_map::Entry<T0, T1>
where
    T0: Clone,
    T1: Clone,
{
    fn clone(&self) -> Self {
        Self {
            key: self.key.clone(),
            value: self.value.clone(),
        }
    }
}

impl<T0, T1> Clone for crate::types::sui_framework::vec_map::VecMap<T0, T1>
where
    T0: Clone,
    T1: Clone,
{
    fn clone(&self) -> Self {
        Self {
            contents: self.contents.clone(),
        }
    }
}

impl<T0> Clone for crate::types::sui_framework::balance::Balance<T0> {
    fn clone(&self) -> Self {
        Self {
            value: self.value,
            phantom_t0: std::marker::PhantomData,
        }
    }
}

impl<K, V> Clone for crate::types::sui_framework::table::Table<K, V> {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            size: self.size,
            phantom_t0: PhantomData,
            phantom_t1: PhantomData,
        }
    }
}

impl<K, V> crate::types::sui_framework::table::Table<K, V> {
    pub fn new(id: sui::types::Address, size: u64) -> Self {
        Self {
            id: sui_address_to_uid(id),
            size,
            phantom_t0: PhantomData,
            phantom_t1: PhantomData,
        }
    }

    pub fn id(&self) -> sui::types::Address {
        self.id.id.bytes
    }

    pub fn size(&self) -> usize {
        usize::try_from(self.size).unwrap_or(usize::MAX)
    }
}

/// Deserialize a Move struct value, tolerating the `{ fields: ... }` wrapper.
#[derive(Clone, Debug)]
pub struct MoveFields<T>(pub T);

impl<'de, T> Deserialize<'de> for MoveFields<T>
where
    T: DeserializeOwned,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        if !deserializer.is_human_readable() {
            return T::deserialize(deserializer).map(Self);
        }

        let value = serde_json::Value::deserialize(deserializer)?;
        let value = super::move_json::strip_fields_owned(value);
        serde_json::from_value::<T>(value)
            .map(Self)
            .map_err(serde::de::Error::custom)
    }
}

/// Deserialize `0x1::option::Option<T>`.
///
/// accepted:
/// - `{ vec: [] | [T] }` (Move stdlib option layout),
/// - `{ some: T }` / `{ none: ... }`,
/// - `null`,
/// - and a best-effort fallback treating the value as `T`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MoveOption<T>(pub Option<T>);

impl<T> From<Option<T>> for MoveOption<T> {
    fn from(value: Option<T>) -> Self {
        Self(value)
    }
}

impl<T> Serialize for MoveOption<T>
where
    T: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if serializer.is_human_readable() {
            return self.0.serialize(serializer);
        }

        let mut seq = serializer.serialize_seq(Some(usize::from(self.0.is_some())))?;
        if let Some(value) = &self.0 {
            seq.serialize_element(value)?;
        }
        seq.end()
    }
}

fn deserialize_move_option_inner<T, E>(value: serde_json::Value) -> Result<T, E>
where
    T: DeserializeOwned,
    E: serde::de::Error,
{
    let value = super::move_json::strip_fields_owned(value);
    match serde_json::from_value::<T>(value.clone()) {
        Ok(parsed) => Ok(parsed),
        Err(original) => {
            if let Ok(parsed) = serde_json::from_value::<PublishedMoveEnum<T>>(value.clone()) {
                return Ok(parsed.0);
            }

            if let serde_json::Value::Object(object) = &value {
                for key in ["value", "inner"] {
                    if let Some(inner) = object.get(key) {
                        match deserialize_move_option_inner::<T, E>(inner.clone()) {
                            Ok(parsed) => return Ok(parsed),
                            Err(_) => continue,
                        }
                    }
                }
            }

            if let Some(bytes) =
                super::move_json::parse_byte_vector_value(&value).map_err(E::custom)?
            {
                let bytes = bytes
                    .into_iter()
                    .map(serde_json::Value::from)
                    .collect::<Vec<_>>();
                return serde_json::from_value::<T>(serde_json::Value::Array(bytes))
                    .map_err(E::custom);
            }

            Err(E::custom(original))
        }
    }
}

pub fn deserialize_published_move_enum<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: DeserializeOwned,
{
    if !deserializer.is_human_readable() {
        return T::deserialize(deserializer);
    }

    let value = serde_json::Value::deserialize(deserializer)?;
    let value = super::move_json::strip_fields_owned(value);
    match serde_json::from_value::<T>(value.clone()) {
        Ok(parsed) => Ok(parsed),
        Err(original) => serde_json::from_value::<PublishedMoveEnum<T>>(value)
            .map(|parsed| parsed.0)
            .map_err(|_| D::Error::custom(original)),
    }
}

impl<'de, T> Deserialize<'de> for MoveOption<T>
where
    T: DeserializeOwned,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // In BCS, Move `Option<T>` is a single-field struct containing a `vector<T>`.
        if !deserializer.is_human_readable() {
            let mut vec = Vec::<T>::deserialize(deserializer)?;
            return Ok(Self(vec.drain(..).next()));
        }

        let value = serde_json::Value::deserialize(deserializer)?;
        let value = super::move_json::strip_fields_owned(value);

        match value {
            serde_json::Value::Null => Ok(Self(None)),
            serde_json::Value::Array(mut vec) => {
                if vec.is_empty() {
                    return Ok(Self(None));
                }

                let direct = serde_json::Value::Array(vec.clone());
                if let Ok(parsed) = deserialize_move_option_inner::<T, D::Error>(direct) {
                    return Ok(Self(Some(parsed)));
                }

                Ok(Self(
                    vec.drain(..)
                        .next()
                        .map(deserialize_move_option_inner::<T, D::Error>)
                        .transpose()
                        .map_err(serde::de::Error::custom)?,
                ))
            }
            serde_json::Value::Object(mut object) => {
                if let Some(vec) = object.remove("vec").or_else(|| object.remove("Vec")) {
                    let vec = super::move_json::strip_fields_owned(vec)
                        .as_array()
                        .cloned()
                        .unwrap_or_default();
                    let mut vec = vec;
                    return Ok(Self(
                        vec.drain(..)
                            .next()
                            .map(deserialize_move_option_inner::<T, D::Error>)
                            .transpose()?,
                    ));
                }

                if let Some(inner) = object.remove("some").or_else(|| object.remove("Some")) {
                    let inner = deserialize_move_option_inner::<T, D::Error>(inner)?;
                    return Ok(Self(Some(inner)));
                }

                if object.contains_key("none") || object.contains_key("None") {
                    return Ok(Self(None));
                }

                // Fallback: treat as `T` directly.
                deserialize_move_option_inner::<T, D::Error>(serde_json::Value::Object(object))
                    .map(Some)
                    .map(Self)
            }
            other => deserialize_move_option_inner::<T, D::Error>(other)
                .map(Some)
                .map(Self),
        }
    }
}

/// Deserialize a published Move enum value by extracting its variant tag and
/// delegating to `T`'s existing string-based enum deserializer.
#[derive(Clone, Debug)]
pub struct PublishedMoveEnum<T>(pub T);

fn published_move_variant_name_owned(value: serde_json::Value) -> Result<String, String> {
    let value = super::move_json::strip_fields_owned(value);

    match value {
        serde_json::Value::String(name) => Ok(name),
        serde_json::Value::Object(mut object) => object
            .remove("_variant_name")
            .or_else(|| object.remove("@variant"))
            .or_else(|| object.remove("variant"))
            .and_then(|value| value.as_str().map(ToOwned::to_owned))
            .or_else(|| {
                if object.len() == 1 {
                    object.keys().next().cloned()
                } else {
                    None
                }
            })
            .ok_or_else(|| "missing published Move enum variant tag".to_string()),
        other => Err(format!("unexpected published Move enum value: {other}")),
    }
}

impl<'de, T> Deserialize<'de> for PublishedMoveEnum<T>
where
    T: DeserializeOwned,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        if !deserializer.is_human_readable() {
            return T::deserialize(deserializer).map(Self);
        }

        let value = serde_json::Value::deserialize(deserializer)?;
        let variant = published_move_variant_name_owned(value).map_err(serde::de::Error::custom)?;

        serde_json::from_value::<T>(serde_json::Value::String(variant))
            .map(Self)
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct MoveString {
    pub bytes: Vec<u8>,
}

impl fmt::Debug for MoveString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("MoveString")
            .field(&String::from_utf8_lossy(&self.bytes))
            .finish()
    }
}

impl Serialize for MoveString {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if serializer.is_human_readable() {
            return String::from_utf8_lossy(&self.bytes).serialize(serializer);
        }
        self.bytes.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for MoveString {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            let value = serde_json::Value::deserialize(deserializer)?;
            if let serde_json::Value::String(text) = &value {
                if let Some(hex) = text.strip_prefix("0x") {
                    if let Ok(bytes) = hex::decode(hex) {
                        if std::str::from_utf8(&bytes).is_ok() {
                            return Ok(Self { bytes });
                        }
                    }
                }

                return Ok(Self {
                    bytes: text.as_bytes().to_vec(),
                });
            }

            if let Some(bytes) = super::move_json::parse_byte_vector_value(&value)
                .map_err(serde::de::Error::custom)?
            {
                if std::str::from_utf8(&bytes).is_ok() {
                    return Ok(Self { bytes });
                }
            }

            super::move_json::parse_string_value(&value)
                .map_err(serde::de::Error::custom)?
                .map(|value| Self {
                    bytes: value.into_bytes(),
                })
                .ok_or_else(|| serde::de::Error::custom("missing Move string value"))
        } else {
            Vec::<u8>::deserialize(deserializer).map(|bytes| Self { bytes })
        }
    }
}

pub struct Ignored<T0 = (), T1 = (), T2 = ()> {
    _marker: PhantomData<(T0, T1, T2)>,
}

impl<T0, T1, T2> Clone for Ignored<T0, T1, T2> {
    fn clone(&self) -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

impl<T0, T1, T2> fmt::Debug for Ignored<T0, T1, T2> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Ignored").finish()
    }
}

impl<T0, T1, T2> PartialEq for Ignored<T0, T1, T2> {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl<T0, T1, T2> Eq for Ignored<T0, T1, T2> {}

impl<T0, T1, T2> Serialize for Ignored<T0, T1, T2> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        ().serialize(serializer)
    }
}

impl<'de, T0, T1, T2> Deserialize<'de> for Ignored<T0, T1, T2> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        if !deserializer.is_human_readable() {
            return Err(serde::de::Error::custom(
                "cannot BCS deserialize an ignored Move field without its layout",
            ));
        }
        <()>::deserialize(deserializer)?;
        Ok(Self {
            _marker: PhantomData,
        })
    }
}

impl From<ID> for sui::types::Address {
    fn from(value: ID) -> Self {
        value.bytes
    }
}

impl From<UID> for sui::types::Address {
    fn from(value: UID) -> Self {
        value.id.bytes
    }
}

impl From<MoveString> for String {
    fn from(value: MoveString) -> Self {
        String::from_utf8_lossy(&value.bytes).into_owned()
    }
}

impl From<&str> for MoveString {
    fn from(value: &str) -> Self {
        Self {
            bytes: value.as_bytes().to_vec(),
        }
    }
}

impl From<String> for MoveString {
    fn from(value: String) -> Self {
        Self {
            bytes: value.into_bytes(),
        }
    }
}

impl MoveString {
    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.bytes).expect("generated Move string must be UTF-8")
    }
}

impl AsRef<str> for MoveString {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for ID {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.bytes.fmt(f)
    }
}

impl MoveType for MoveString {
    fn type_tag_static() -> sui::types::TypeTag {
        sui::types::TypeTag::Struct(Box::new(Self::struct_tag_static()))
    }
}

impl MoveStruct for MoveString {
    fn struct_tag_static() -> sui::types::StructTag {
        struct_tag("0x1", "string", "String", vec![])
    }
}

impl<T> MoveType for MoveOption<T>
where
    T: MoveType,
{
    fn type_tag_static() -> sui::types::TypeTag {
        sui::types::TypeTag::Struct(Box::new(Self::struct_tag_static()))
    }
}

impl<T> MoveStruct for MoveOption<T>
where
    T: MoveType,
{
    fn struct_tag_static() -> sui::types::StructTag {
        struct_tag("0x1", "option", "Option", vec![T::type_tag_static()])
    }
}

impl MoveType for TypeName {
    fn type_tag_static() -> sui::types::TypeTag {
        sui::types::TypeTag::Struct(Box::new(Self::struct_tag_static()))
    }
}

impl MoveStruct for TypeName {
    fn struct_tag_static() -> sui::types::StructTag {
        struct_tag("0x1", "type_name", "TypeName", vec![])
    }
}

impl<T0, T1, T2> MoveType for Ignored<T0, T1, T2> {
    fn type_tag_static() -> sui::types::TypeTag {
        sui::types::TypeTag::Bool
    }
}

impl<T0, T1, T2> HasCopy for Ignored<T0, T1, T2> {}
impl<T0, T1, T2> HasDrop for Ignored<T0, T1, T2> {}
impl<T0, T1, T2> HasStore for Ignored<T0, T1, T2> {}
impl<T0, T1> HasCopy for crate::types::sui_framework::vec_map::VecMap<T0, T1> where Self: Clone {}
impl HasCopy for MoveString {}
impl HasDrop for MoveString {}
impl HasStore for MoveString {}
impl<T: MoveType> HasStore for MoveOption<T> {}
impl<T: MoveType + Clone> HasCopy for MoveOption<T> {}
impl<T: MoveType> HasDrop for MoveOption<T> {}
impl HasCopy for TypeName {}
impl HasDrop for TypeName {}
impl HasStore for TypeName {}

fn struct_tag(
    address: &'static str,
    module: &'static str,
    name: &'static str,
    type_params: Vec<sui::types::TypeTag>,
) -> sui::types::StructTag {
    let _ = PhantomData::<()>;
    sui::types::StructTag::new(
        sui::types::Address::from_static(address),
        sui::types::Identifier::from_static(module),
        sui::types::Identifier::from_static(name),
        type_params,
    )
}

#[cfg(test)]
mod tests {
    use {super::*, serde_json::json};

    fn address(value: &'static str) -> sui::types::Address {
        sui::types::Address::from_static(value)
    }

    fn id(value: &'static str) -> ID {
        ID {
            bytes: address(value),
        }
    }

    fn uid_for_test(value: &'static str) -> crate::types::sui_framework::object::UID {
        crate::types::sui_framework::object::UID {
            id: crate::types::sui_framework::object::ID {
                bytes: address(value),
            },
        }
    }

    #[test]
    fn move_string_helpers_cover_human_readable_and_bcs_serde() {
        let value = MoveString::from("nexus");

        assert_eq!(value.as_str(), "nexus");
        assert_eq!(value.as_ref(), "nexus");
        assert_eq!(String::from(value.clone()), "nexus");
        assert_eq!(MoveString::from(String::from("sdk")).as_str(), "sdk");
        assert_eq!(format!("{value:?}"), "MoveString(\"nexus\")");
        assert_eq!(serde_json::to_value(&value).unwrap(), json!("nexus"));
        assert_eq!(
            serde_json::from_value::<MoveString>(json!("nexus")).unwrap(),
            value
        );

        let bytes = bcs::to_bytes(&value).unwrap();
        assert_eq!(bcs::from_bytes::<MoveString>(&bytes).unwrap(), value);
    }

    #[test]
    fn type_name_deserializes_sui_json_string() {
        let parsed: TypeName =
            serde_json::from_value(json!("0xa5::scheduler::QueueGeneratorWitness")).unwrap();

        assert_eq!(
            parsed,
            TypeName::new("0xa5::scheduler::QueueGeneratorWitness")
        );
    }

    #[test]
    fn id_and_uid_helpers_return_addresses_and_display_id() {
        let id = id("0x123");
        let uid = UID { id: id.clone() };

        assert_eq!(format!("{id}"), id.bytes.to_string());
        assert_eq!(sui::types::Address::from(id.clone()), id.bytes);
        assert_eq!(sui::types::Address::from(uid), id.bytes);
    }

    #[test]
    fn id_deserializes_human_readable_address_shapes_and_bcs() {
        let id_from_string: ID = serde_json::from_value(json!("0xda6")).unwrap();
        assert_eq!(id_from_string.bytes, address("0xda6"));

        let id_from_object: ID = serde_json::from_value(json!({ "bytes": "0xda6" })).unwrap();
        assert_eq!(id_from_object, id_from_string);

        let bytes = bcs::to_bytes(&id_from_string).unwrap();
        assert_eq!(bcs::from_bytes::<ID>(&bytes).unwrap(), id_from_string);
    }

    #[test]
    fn ignored_is_a_json_only_unit_shim() {
        let ignored = Ignored::<ID, UID, MoveString> {
            _marker: PhantomData,
        };
        let other_ignored = ignored.clone();
        assert_eq!(format!("{ignored:?}"), "Ignored");
        assert_eq!(ignored, other_ignored);
        assert_eq!(serde_json::to_value(&ignored).unwrap(), json!(null));
        let decoded_ignored: Ignored<ID, UID, MoveString> =
            serde_json::from_value(json!(null)).unwrap();
        assert_eq!(decoded_ignored, ignored);
        let ignored_bytes = bcs::to_bytes(&ignored).unwrap();
        assert!(bcs::from_bytes::<Ignored<ID, UID, MoveString>>(&ignored_bytes).is_err());
    }

    #[test]
    fn framework_types_consume_bcs_layout_bytes() {
        use crate::types::sui_framework::{
            balance,
            object_table,
            sui as sui_module,
            table_vec,
            vec_map,
        };

        #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
        struct BalanceThenSentinel {
            balance: balance::Balance<sui_module::SUI>,
            sentinel: u8,
        }

        let balance = BalanceThenSentinel {
            balance: balance::Balance {
                value: 42,
                phantom_t0: PhantomData,
            },
            sentinel: 7,
        };
        let balance_bytes = bcs::to_bytes(&balance).unwrap();
        assert_eq!(
            bcs::from_bytes::<BalanceThenSentinel>(&balance_bytes).unwrap(),
            balance
        );

        #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
        struct VecMapThenSentinel {
            map: vec_map::VecMap<u8, u64>,
            sentinel: u8,
        }

        let map = VecMapThenSentinel {
            map: vec_map::VecMap {
                contents: vec![vec_map::Entry { key: 1, value: 9 }],
            },
            sentinel: 8,
        };
        let map_bytes = bcs::to_bytes(&map).unwrap();
        assert_eq!(
            bcs::from_bytes::<VecMapThenSentinel>(&map_bytes).unwrap(),
            map
        );

        #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
        struct ObjectTableThenSentinel {
            table: object_table::ObjectTable<ID, UID>,
            sentinel: u8,
        }

        let object_table = ObjectTableThenSentinel {
            table: object_table::ObjectTable {
                id: uid_for_test("0x456"),
                size: 3,
                phantom_t0: PhantomData,
                phantom_t1: PhantomData,
            },
            sentinel: 9,
        };
        let object_table_bytes = bcs::to_bytes(&object_table).unwrap();
        assert_eq!(
            bcs::from_bytes::<ObjectTableThenSentinel>(&object_table_bytes).unwrap(),
            object_table
        );

        #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
        struct TableVecThenSentinel {
            table_vec: table_vec::TableVec<u64>,
            sentinel: u8,
        }

        let table_vec = TableVecThenSentinel {
            table_vec: table_vec::TableVec {
                contents: MoveTable::new(address("0x789"), 4),
                phantom_t0: PhantomData,
            },
            sentinel: 10,
        };
        let table_vec_bytes = bcs::to_bytes(&table_vec).unwrap();
        assert_eq!(
            bcs::from_bytes::<TableVecThenSentinel>(&table_vec_bytes).unwrap(),
            table_vec
        );
    }

    #[test]
    fn support_move_tags_match_sui_framework_types() {
        fn assert_struct_type<T>()
        where
            T: MoveStruct + MoveType,
        {
            assert!(matches!(
                T::type_tag_static(),
                sui::types::TypeTag::Struct(_)
            ));
        }

        assert_struct_type::<ID>();
        assert_struct_type::<UID>();
        assert_struct_type::<Bag>();
        assert_struct_type::<ObjectBag>();
        assert_struct_type::<PriorityQueue<ID>>();
        assert_struct_type::<MoveString>();
        assert_struct_type::<MoveOption<ID>>();
        assert_struct_type::<MoveVecSet<ID>>();
        assert_struct_type::<MoveTable<ID, UID>>();
        assert_struct_type::<TypeName>();

        assert_eq!(ID::struct_tag_static().module().as_str(), "object");
        assert_eq!(UID::struct_tag_static().name().as_str(), "UID");
        assert_eq!(Bag::struct_tag_static().module().as_str(), "bag");
        assert_eq!(
            ObjectBag::struct_tag_static().module().as_str(),
            "object_bag"
        );
        assert_eq!(
            PriorityQueue::<ID>::struct_tag_static().type_params(),
            &[ID::type_tag_static()]
        );
        assert_eq!(MoveString::struct_tag_static().module().as_str(), "string");
        assert_eq!(
            MoveOption::<ID>::struct_tag_static().type_params(),
            &[ID::type_tag_static()]
        );
        assert_eq!(
            MoveVecSet::<ID>::struct_tag_static().type_params(),
            &[ID::type_tag_static()]
        );
        assert_eq!(
            MoveTable::<ID, UID>::struct_tag_static().type_params(),
            &[ID::type_tag_static(), UID::type_tag_static()]
        );
        assert_eq!(TypeName::struct_tag_static().module().as_str(), "type_name");
        assert_eq!(
            Ignored::<ID, UID, MoveString>::type_tag_static(),
            sui::types::TypeTag::Bool
        );
    }

    #[test]
    fn support_structs_round_trip_through_serde() {
        let bag = Bag {
            id: uid_for_test("0x456"),
            size: 9,
        };
        let object_bag = ObjectBag {
            id: uid_for_test("0x789"),
            size: 11,
        };

        assert_eq!(
            serde_json::from_value::<Bag>(serde_json::to_value(&bag).unwrap()).unwrap(),
            bag
        );
        assert_eq!(
            serde_json::from_value::<ObjectBag>(serde_json::to_value(&object_bag).unwrap())
                .unwrap(),
            object_bag
        );
    }
}
