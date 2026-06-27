use {
    super::{MoveOption, MoveTable, MoveVecSet, TypeName},
    crate::sui,
    serde::{Deserialize, Serialize},
    std::{fmt, marker::PhantomData},
    sui_move::{HasCopy, HasDrop, HasKey, HasStore, MoveStruct, MoveType},
};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ID {
    pub bytes: sui::types::Address,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UID {
    pub id: ID,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bag {
    pub id: UID,
    pub size: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectBag {
    pub id: UID,
    pub size: u64,
}

pub struct PriorityQueue<T> {
    _marker: PhantomData<T>,
}

impl<T> Clone for PriorityQueue<T> {
    fn clone(&self) -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

impl<T> fmt::Debug for PriorityQueue<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PriorityQueue").finish()
    }
}

impl<T> PartialEq for PriorityQueue<T> {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl<T> Eq for PriorityQueue<T> {}

impl<T> Serialize for PriorityQueue<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        ().serialize(serializer)
    }
}

impl<'de, T> Deserialize<'de> for PriorityQueue<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let _ = <()>::deserialize(deserializer)?;
        Ok(Self {
            _marker: PhantomData,
        })
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
            return String::deserialize(deserializer).map(|value| Self {
                bytes: value.into_bytes(),
            });
        }
        Vec::<u8>::deserialize(deserializer).map(|bytes| Self { bytes })
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
        let _ = <()>::deserialize(deserializer)?;
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

impl MoveType for ID {
    fn type_tag_static() -> sui::types::TypeTag {
        sui::types::TypeTag::Struct(Box::new(struct_tag("0x2", "object", "ID", vec![])))
    }
}

impl MoveStruct for ID {
    fn struct_tag_static() -> sui::types::StructTag {
        struct_tag("0x2", "object", "ID", vec![])
    }
}

impl MoveType for UID {
    fn type_tag_static() -> sui::types::TypeTag {
        sui::types::TypeTag::Struct(Box::new(Self::struct_tag_static()))
    }
}

impl MoveStruct for UID {
    fn struct_tag_static() -> sui::types::StructTag {
        struct_tag("0x2", "object", "UID", vec![])
    }
}

impl MoveType for Bag {
    fn type_tag_static() -> sui::types::TypeTag {
        sui::types::TypeTag::Struct(Box::new(Self::struct_tag_static()))
    }
}

impl MoveStruct for Bag {
    fn struct_tag_static() -> sui::types::StructTag {
        struct_tag("0x2", "bag", "Bag", vec![])
    }
}

impl MoveType for ObjectBag {
    fn type_tag_static() -> sui::types::TypeTag {
        sui::types::TypeTag::Struct(Box::new(Self::struct_tag_static()))
    }
}

impl MoveStruct for ObjectBag {
    fn struct_tag_static() -> sui::types::StructTag {
        struct_tag("0x2", "object_bag", "ObjectBag", vec![])
    }
}

impl<T> MoveType for PriorityQueue<T>
where
    T: MoveType,
{
    fn type_tag_static() -> sui::types::TypeTag {
        sui::types::TypeTag::Struct(Box::new(Self::struct_tag_static()))
    }
}

impl<T> MoveStruct for PriorityQueue<T>
where
    T: MoveType,
{
    fn struct_tag_static() -> sui::types::StructTag {
        struct_tag(
            "0x2",
            "priority_queue",
            "PriorityQueue",
            vec![T::type_tag_static()],
        )
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

impl<T> MoveType for MoveVecSet<T>
where
    T: MoveType,
{
    fn type_tag_static() -> sui::types::TypeTag {
        sui::types::TypeTag::Struct(Box::new(Self::struct_tag_static()))
    }
}

impl<T> MoveStruct for MoveVecSet<T>
where
    T: MoveType,
{
    fn struct_tag_static() -> sui::types::StructTag {
        struct_tag("0x2", "vec_set", "VecSet", vec![T::type_tag_static()])
    }
}

impl<K, V> MoveType for MoveTable<K, V>
where
    K: MoveType + fmt::Debug + PartialEq + Eq,
    V: MoveType + fmt::Debug + PartialEq + Eq,
{
    fn type_tag_static() -> sui::types::TypeTag {
        sui::types::TypeTag::Struct(Box::new(Self::struct_tag_static()))
    }
}

impl<K, V> MoveStruct for MoveTable<K, V>
where
    K: MoveType + fmt::Debug + PartialEq + Eq,
    V: MoveType + fmt::Debug + PartialEq + Eq,
{
    fn struct_tag_static() -> sui::types::StructTag {
        struct_tag(
            "0x2",
            "table",
            "Table",
            vec![K::type_tag_static(), V::type_tag_static()],
        )
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
impl HasCopy for ID {}
impl HasDrop for ID {}
impl HasStore for ID {}
impl HasStore for UID {}
impl HasKey for Bag {}
impl HasStore for Bag {}
impl HasKey for ObjectBag {}
impl HasStore for ObjectBag {}
impl<T: MoveType> HasStore for PriorityQueue<T> {}
impl HasCopy for MoveString {}
impl HasDrop for MoveString {}
impl HasStore for MoveString {}
impl<T: MoveType> HasStore for MoveOption<T> {}
impl<T: MoveType + Clone> HasCopy for MoveOption<T> {}
impl<T: MoveType> HasDrop for MoveOption<T> {}
impl<T: MoveType> HasStore for MoveVecSet<T> {}
impl<T: MoveType + Clone> HasCopy for MoveVecSet<T> {}
impl<T: MoveType> HasDrop for MoveVecSet<T> {}
impl<K, V> HasKey for MoveTable<K, V> {}
impl<K, V> HasStore for MoveTable<K, V> {}
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

    fn uid(value: &'static str) -> UID {
        UID { id: id(value) }
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
    fn id_and_uid_helpers_return_addresses_and_display_id() {
        let id = id("0x123");
        let uid = UID { id: id.clone() };

        assert_eq!(format!("{id}"), id.bytes.to_string());
        assert_eq!(sui::types::Address::from(id.clone()), id.bytes);
        assert_eq!(sui::types::Address::from(uid), id.bytes);
    }

    #[test]
    fn priority_queue_and_ignored_are_unit_shims_for_serde_and_equality() {
        let queue = PriorityQueue::<ID> {
            _marker: PhantomData,
        };
        let other_queue = queue.clone();
        assert_eq!(format!("{queue:?}"), "PriorityQueue");
        assert_eq!(queue, other_queue);
        assert_eq!(serde_json::to_value(&queue).unwrap(), json!(null));
        let decoded_queue: PriorityQueue<ID> = serde_json::from_value(json!(null)).unwrap();
        assert_eq!(decoded_queue, queue);
        let queue_bytes = bcs::to_bytes(&queue).unwrap();
        assert_eq!(
            bcs::from_bytes::<PriorityQueue<ID>>(&queue_bytes).unwrap(),
            queue
        );

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
        assert_eq!(
            bcs::from_bytes::<Ignored<ID, UID, MoveString>>(&ignored_bytes).unwrap(),
            ignored
        );
    }

    #[test]
    fn generated_support_move_tags_match_sui_framework_types() {
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
    fn generated_support_structs_round_trip_through_serde() {
        let bag = Bag {
            id: uid("0x456"),
            size: 9,
        };
        let object_bag = ObjectBag {
            id: uid("0x789"),
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
